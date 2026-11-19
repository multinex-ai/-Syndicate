//! Apple Silicon Metal (`objc2-metal` 0.3) hardware working-set telemetry and
//! untracked `MTLHeap` transient buffer aliasing for low-memory unified DRAM systems.
use crate::GpuDeviceProbe;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBuffer, MTLCreateSystemDefaultDevice, MTLDevice, MTLHazardTrackingMode, MTLHeap,
    MTLHeapDescriptor, MTLHeapType, MTLResourceOptions, MTLStorageMode,
};

pub fn probe_metal() -> Option<GpuDeviceProbe> {
    let device = MTLCreateSystemDefaultDevice()?;
    let name = device.name().to_string();
    let working_set_max = device.recommendedMaxWorkingSetSize();
    let allocated = device.currentAllocatedSize() as u64;
    let max_buffer = device.maxBufferLength() as u64;
    let unified = device.hasUnifiedMemory();
    let budget_free = working_set_max.saturating_sub(allocated);
    Some(GpuDeviceProbe {
        backend: "metal",
        device_name: name,
        unified_memory: unified,
        total_or_working_set_bytes: working_set_max,
        free_budget_bytes: budget_free,
        allocated_bytes: allocated,
        max_buffer_bytes: max_buffer,
        simd_width: 32,
        cooperative_matrix: true, // Apple simdgroup_matrix hardware
    })
}

/// Untracked private Metal heap for aliasing transient stage buffers (encode -> DiT -> tiled VAE)
/// inside a single fixed allocation without duplicating peak unified memory.
/// Uses `StorageModePrivate` (enabling Apple GPU lossless bandwidth compression) and
/// `HazardTrackingModeUntracked` (eliminating driver hazard-tracking overhead between stages).
pub struct MetalTransientArena {
    heap: Retained<ProtocolObject<dyn MTLHeap>>,
    capacity_bytes: usize,
}

impl MetalTransientArena {
    pub fn new(capacity_bytes: usize) -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("No Metal device available")?;
        let max_ws = device.recommendedMaxWorkingSetSize() as usize;
        if capacity_bytes == 0 || capacity_bytes > max_ws {
            return Err(format!(
                "Requested transient arena ({capacity_bytes} B) exceeds Metal working set ({max_ws} B)"
            ));
        }
        let desc = MTLHeapDescriptor::new();
        desc.setType(MTLHeapType::Placement);
        desc.setSize(capacity_bytes);
        desc.setStorageMode(MTLStorageMode::Private);
        desc.setHazardTrackingMode(MTLHazardTrackingMode::Untracked);
        let heap = device
            .newHeapWithDescriptor(&desc)
            .ok_or("Metal failed to allocate placement heap")?;
        Ok(Self {
            heap,
            capacity_bytes,
        })
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    /// Alias a stage buffer at `offset` (256-byte aligned) with zero extra heap growth.
    /// Non-overlapping stages (e.g. text-encoder activations vs VAE decode tiles) share `offset = 0`.
    pub fn alias_buffer(
        &self,
        length: usize,
        offset: usize,
    ) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>, String> {
        let aligned_offset = (offset + 255) & !255;
        if length == 0 || aligned_offset.saturating_add(length) > self.capacity_bytes {
            return Err("Transient stage buffer exceeds aliased Metal heap bounds".into());
        }
        let opts =
            MTLResourceOptions::StorageModePrivate | MTLResourceOptions::HazardTrackingModeUntracked;
        // SAFETY: `aligned_offset` is 256-byte aligned and `aligned_offset + length <= capacity_bytes`.
        unsafe { self.heap.newBufferWithLength_options_offset(length, opts, aligned_offset) }
            .ok_or_else(|| "Failed to alias transient buffer on Metal placement heap".into())
    }
}
