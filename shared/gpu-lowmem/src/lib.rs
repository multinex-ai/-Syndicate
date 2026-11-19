//! Low-level graphics & compute API telemetry + deterministic low-memory execution planner.
//!
//! Priority order:
//!   1. Apple Silicon native Metal (`objc2-metal` 0.3 `recommendedMaxWorkingSetSize` + `MTLHeap` aliasing)
//!   2. NVIDIA CUDA Driver API (`libcuda.so.1` `cuMemGetInfo_v2` + Tensor Core capability)
//!   3. Vulkan 1.2+ (`ash` 0.38 `VK_EXT_memory_budget` + `VK_KHR_cooperative_matrix`)
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
pub mod metal_backend;
pub mod vulkan_cuda;

pub const GIB: u64 = 1 << 30;
pub const MIB: u64 = 1 << 20;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuDeviceProbe {
    pub backend: &'static str,
    pub device_name: String,
    pub unified_memory: bool,
    pub total_or_working_set_bytes: u64,
    pub free_budget_bytes: u64,
    pub allocated_bytes: u64,
    pub max_buffer_bytes: u64,
    pub simd_width: u32,
    pub cooperative_matrix: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// < 6 GiB GPU working set (8 GB Macs, 4-6 GB laptops/APUs): aggressive stage offload + q4_0 KV + 256px tiles
    UltraLow,
    /// 6..15 GiB GPU working set (16-18 GB M-series Macs, 8-16 GB discrete GPUs): model stage offload + 512px VAE tiles
    Constrained,
    /// 15..32 GiB GPU working set (24 GB Ada L4 / RTX 4090 / 32 GB Mac): resident quantized/FP8 + tiled >1024px VAE
    Balanced,
    /// >= 32 GiB GPU working set (48 GB A40 / 80 GB A100 / 96 GB Blackwell): full GPU residency, zero offload overhead
    HighHeadroom,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LowMemExecutionPlan {
    pub tier: MemoryTier,
    pub effective_budget_bytes: u64,
    pub llama_low_memory: bool,
    pub llama_kv_cache_type: &'static str,
    pub llama_batch_size: u32,
    pub llama_ubatch_size: u32,
    pub llama_cache_ram_mib: u32,
    pub llama_gpu_layers: u32,
    /// "sequential_cpu" | "model_cpu" | "none"
    pub diffusion_offload: &'static str,
    pub vae_tiling: bool,
    pub vae_tile_sample_min_px: u32,
    pub vae_slicing: bool,
    pub attention_slicing: bool,
    pub max_image_dim: u32,
    pub cuda_alloc_conf: &'static str,
}

/// Probe the best native low-level graphics/compute backend available on this host.
pub fn probe_best_gpu() -> Option<GpuDeviceProbe> {
    #[cfg(target_os = "macos")]
    if let Some(m) = metal_backend::probe_metal() {
        return Some(m);
    }
    if let Some(c) = vulkan_cuda::probe_cuda() {
        return Some(c);
    }
    vulkan_cuda::probe_vulkan()
}

/// Compute a deterministic low-memory execution plan from GPU working-set telemetry and host free RAM.
pub fn plan_for_budget(
    gpu: Option<&GpuDeviceProbe>,
    host_available_bytes: Option<u64>,
    estimated_model_bytes: u64,
) -> LowMemExecutionPlan {
    let effective = match (gpu, host_available_bytes) {
        (Some(g), Some(h)) if g.unified_memory => {
            // On Apple Silicon unified DRAM, respect both Metal's wired working-set ceiling and free OS pages,
            // while allowing mmap'd read-only model pages up to recommendedMaxWorkingSetSize.
            g.free_budget_bytes.min(h.saturating_add(2 * GIB))
        }
        (Some(g), _) => g.free_budget_bytes,
        (None, Some(h)) => h,
        (None, None) => 8 * GIB,
    };

    let tier = if effective < 6 * GIB {
        MemoryTier::UltraLow
    } else if effective < 15 * GIB {
        MemoryTier::Constrained
    } else if effective < 32 * GIB {
        MemoryTier::Balanced
    } else {
        MemoryTier::HighHeadroom
    };

    // Compute safe partial GPU layer offload for discrete low-VRAM cards (e.g. 4-8 GB Vulkan/CUDA)
    let gpu_layers = match gpu {
        Some(g) if !g.unified_memory && estimated_model_bytes > 0 && g.free_budget_bytes < estimated_model_bytes => {
            let usable = g.free_budget_bytes.saturating_sub(512 * MIB);
            let ratio = (usable as f64 / estimated_model_bytes as f64).clamp(0.0, 1.0);
            ((ratio * 40.0).floor() as u32).max(1)
        }
        _ => 99,
    };

    match tier {
        MemoryTier::UltraLow => LowMemExecutionPlan {
            tier,
            effective_budget_bytes: effective,
            llama_low_memory: true,
            llama_kv_cache_type: "q4_0",
            llama_batch_size: 128,
            llama_ubatch_size: 32,
            llama_cache_ram_mib: 512,
            llama_gpu_layers: gpu_layers,
            diffusion_offload: "sequential_cpu",
            vae_tiling: true,
            vae_tile_sample_min_px: 256,
            vae_slicing: true,
            attention_slicing: true,
            max_image_dim: 768,
            cuda_alloc_conf: "expandable_segments:True,garbage_collection_threshold:0.6,max_split_size_mb:128",
        },
        MemoryTier::Constrained => LowMemExecutionPlan {
            tier,
            effective_budget_bytes: effective,
            llama_low_memory: true,
            llama_kv_cache_type: "q8_0",
            llama_batch_size: 256,
            llama_ubatch_size: 64,
            llama_cache_ram_mib: 1024,
            llama_gpu_layers: gpu_layers,
            diffusion_offload: "model_cpu",
            vae_tiling: true,
            vae_tile_sample_min_px: 512,
            vae_slicing: true,
            attention_slicing: true,
            max_image_dim: 1024,
            cuda_alloc_conf: "expandable_segments:True,garbage_collection_threshold:0.75,max_split_size_mb:256",
        },
        MemoryTier::Balanced => LowMemExecutionPlan {
            tier,
            effective_budget_bytes: effective,
            llama_low_memory: false,
            llama_kv_cache_type: "q8_0",
            llama_batch_size: 512,
            llama_ubatch_size: 128,
            llama_cache_ram_mib: 2048,
            llama_gpu_layers: 99,
            diffusion_offload: if estimated_model_bytes > 20 * GIB {
                "model_cpu"
            } else {
                "none"
            },
            vae_tiling: true,
            vae_tile_sample_min_px: 768,
            vae_slicing: true,
            attention_slicing: false,
            max_image_dim: 1536,
            cuda_alloc_conf: "expandable_segments:True,garbage_collection_threshold:0.8",
        },
        MemoryTier::HighHeadroom => LowMemExecutionPlan {
            tier,
            effective_budget_bytes: effective,
            llama_low_memory: false,
            llama_kv_cache_type: "q8_0",
            llama_batch_size: 2048,
            llama_ubatch_size: 512,
            llama_cache_ram_mib: 4096,
            llama_gpu_layers: 99,
            diffusion_offload: "none",
            vae_tiling: false,
            vae_tile_sample_min_px: 1024,
            vae_slicing: false,
            attention_slicing: false,
            max_image_dim: 1536,
            cuda_alloc_conf: "expandable_segments:True",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultra_low_discrete_gpu_computes_partial_layers_and_sequential_offload() {
        let gpu = GpuDeviceProbe {
            backend: "vulkan",
            device_name: "AMD Custom APU 0405".into(),
            unified_memory: false,
            total_or_working_set_bytes: 4 * GIB,
            free_budget_bytes: 3 * GIB,
            allocated_bytes: GIB,
            max_buffer_bytes: 2 * GIB,
            simd_width: 32,
            cooperative_matrix: false,
        };
        let plan = plan_for_budget(Some(&gpu), Some(8 * GIB), 7 * GIB);
        assert_eq!(plan.tier, MemoryTier::UltraLow);
        assert_eq!(plan.llama_kv_cache_type, "q4_0");
        assert!(plan.llama_gpu_layers < 40 && plan.llama_gpu_layers >= 10);
        assert_eq!(plan.diffusion_offload, "sequential_cpu");
        assert!(plan.vae_tiling && plan.vae_slicing && plan.attention_slicing);
    }

    #[test]
    fn twenty_four_gib_card_offloads_25gib_chroma_but_keeps_smaller_models_resident() {
        let ada24 = GpuDeviceProbe {
            backend: "cuda",
            device_name: "NVIDIA L4".into(),
            unified_memory: false,
            total_or_working_set_bytes: 24 * GIB,
            free_budget_bytes: 22 * GIB,
            allocated_bytes: 2 * GIB,
            max_buffer_bytes: 24 * GIB,
            simd_width: 32,
            cooperative_matrix: true,
        };
        let chroma_plan = plan_for_budget(Some(&ada24), None, 26 * GIB);
        assert_eq!(chroma_plan.tier, MemoryTier::Balanced);
        assert_eq!(chroma_plan.diffusion_offload, "model_cpu");
        assert!(chroma_plan.vae_tiling);

        let small_plan = plan_for_budget(Some(&ada24), None, 9 * GIB);
        assert_eq!(small_plan.diffusion_offload, "none");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_apple_silicon_metal_probe_and_aliased_placement_heap() {
        use objc2_metal::MTLBuffer;
        let probe = probe_best_gpu().expect("Metal GPU must be detected on macOS");
        assert_eq!(probe.backend, "metal");
        assert!(probe.unified_memory);
        assert!(probe.total_or_working_set_bytes > 4 * GIB);

        // Allocate one 4 MiB untracked private placement heap and alias two 4 MiB stage buffers at offset 0!
        let arena = metal_backend::MetalTransientArena::new(4 * MIB as usize)
            .expect("Metal placement heap creation");
        let stage_encode = arena.alias_buffer(4 * MIB as usize, 0).expect("stage 1 alias");
        let stage_vae = arena.alias_buffer(4 * MIB as usize, 0).expect("stage 2 alias");
        assert_eq!(stage_encode.length(), 4 * MIB as usize);
        assert_eq!(stage_vae.length(), 4 * MIB as usize);
        assert_eq!(arena.capacity_bytes(), 4 * MIB as usize);
    }
}
