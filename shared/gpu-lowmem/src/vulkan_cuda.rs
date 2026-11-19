//! Dynamic zero-panic CUDA Driver API (`libcuda`) and Vulkan (`ash` 0.38 + `VK_EXT_memory_budget`)
//! hardware probes for Linux, WSL, Steam Deck, and hybrid Runpod GPUs.
use crate::GpuDeviceProbe;
use ash::vk;
use std::ffi::CStr;
#[cfg(not(target_os = "macos"))]
use std::ffi::CString;

/// Probe NVIDIA CUDA Driver API dynamically via `dlopen` without build-time toolkit dependencies
/// and without panicking on non-NVIDIA hosts.
pub fn probe_cuda() -> Option<GpuDeviceProbe> {
    #[cfg(target_os = "macos")]
    {
        return None;
    }
    #[cfg(not(target_os = "macos"))]
    unsafe {
        let soname = CString::new("libcuda.so.1").ok()?;
        let lib = libc::dlopen(soname.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if lib.is_null() {
            return None;
        }
        type CuInit = unsafe extern "C" fn(u32) -> i32;
        type CuDeviceGetCount = unsafe extern "C" fn(*mut i32) -> i32;
        type CuDeviceGet = unsafe extern "C" fn(*mut i32, i32) -> i32;
        type CuDeviceGetName = unsafe extern "C" fn(*mut libc::c_char, i32, i32) -> i32;
        type CuDeviceGetAttribute = unsafe extern "C" fn(*mut i32, i32, i32) -> i32;
        type CuCtxCreate = unsafe extern "C" fn(*mut *mut libc::c_void, u32, i32) -> i32;
        type CuCtxDestroy = unsafe extern "C" fn(*mut libc::c_void) -> i32;
        type CuMemGetInfo = unsafe extern "C" fn(*mut usize, *mut usize) -> i32;

        let sym = |name: &[u8]| -> Option<*mut libc::c_void> {
            let ptr = libc::dlsym(lib, name.as_ptr().cast());
            (!ptr.is_null()).then_some(ptr)
        };
        let cu_init: CuInit = std::mem::transmute(sym(b"cuInit\0")?);
        let cu_count: CuDeviceGetCount = std::mem::transmute(sym(b"cuDeviceGetCount\0")?);
        let cu_dev: CuDeviceGet = std::mem::transmute(sym(b"cuDeviceGet\0")?);
        let cu_name: CuDeviceGetName = std::mem::transmute(sym(b"cuDeviceGetName\0")?);
        let cu_attr: CuDeviceGetAttribute = std::mem::transmute(sym(b"cuDeviceGetAttribute\0")?);
        let cu_ctx_create: CuCtxCreate = std::mem::transmute(sym(b"cuCtxCreate_v2\0")?);
        let cu_ctx_destroy: CuCtxDestroy = std::mem::transmute(sym(b"cuCtxDestroy_v2\0")?);
        let cu_mem_info: CuMemGetInfo = std::mem::transmute(sym(b"cuMemGetInfo_v2\0")?);

        if cu_init(0) != 0 {
            libc::dlclose(lib);
            return None;
        }
        let mut count = 0i32;
        if cu_count(&mut count) != 0 || count <= 0 {
            libc::dlclose(lib);
            return None;
        }
        let mut dev = 0i32;
        if cu_dev(&mut dev, 0) != 0 {
            libc::dlclose(lib);
            return None;
        }
        let mut raw_name = [0i8; 128];
        let _ = cu_name(raw_name.as_mut_ptr(), 128, dev);
        let name = CStr::from_ptr(raw_name.as_ptr())
            .to_string_lossy()
            .into_owned();

        // CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR = 75, UNIFIED_ADDRESSING = 41
        let mut cc_major = 0i32;
        let _ = cu_attr(&mut cc_major, 75, dev);
        let mut ctx: *mut libc::c_void = std::ptr::null_mut();
        let (mut free, mut total) = (0usize, 0usize);
        if cu_ctx_create(&mut ctx, 0, dev) == 0 {
            let _ = cu_mem_info(&mut free, &mut total);
            let _ = cu_ctx_destroy(ctx);
        }
        libc::dlclose(lib);
        if total == 0 {
            return None;
        }
        Some(GpuDeviceProbe {
            backend: "cuda",
            device_name: name,
            unified_memory: false,
            total_or_working_set_bytes: total as u64,
            free_budget_bytes: free as u64,
            allocated_bytes: total.saturating_sub(free) as u64,
            max_buffer_bytes: total as u64,
            simd_width: 32,
            cooperative_matrix: cc_major >= 7, // Volta+ Tensor Cores (WMMA/MMA)
        })
    }
}

/// Probe Vulkan 1.2/1.3/1.4 physical devices via `ash` dynamic loader (`VK_EXT_memory_budget`
/// + `VK_KHR_portability_enumeration` on macOS KosmicKrisp/MoltenVK + `VK_KHR_cooperative_matrix`).
pub fn probe_vulkan() -> Option<GpuDeviceProbe> {
    unsafe {
        let entry = ash::Entry::load().ok()?;
        let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
        let ext_names = if cfg!(target_os = "macos") {
            vec![vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr()]
        } else {
            Vec::new()
        };
        let flags = if cfg!(target_os = "macos") {
            vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            vk::InstanceCreateFlags::empty()
        };
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&ext_names)
            .flags(flags);
        let instance = entry.create_instance(&create_info, None).ok()?;
        let pdevs = instance.enumerate_physical_devices().ok()?;
        let pdev = *pdevs.first()?;

        let props = instance.get_physical_device_properties(pdev);
        let name = CStr::from_ptr(props.device_name.as_ptr())
            .to_string_lossy()
            .into_owned();
        let exts = instance
            .enumerate_device_extension_properties(pdev)
            .unwrap_or_default();
        let has_ext = |target: &CStr| -> bool {
            exts.iter()
                .any(|e| CStr::from_ptr(e.extension_name.as_ptr()) == target)
        };
        let has_budget_ext = has_ext(vk::EXT_MEMORY_BUDGET_NAME);
        let has_coop_mat = has_ext(c"VK_KHR_cooperative_matrix");

        let mem_props = instance.get_physical_device_memory_properties(pdev);
        let heap_count = mem_props.memory_heap_count as usize;

        let mut total_local = 0u64;
        let mut free_budget = 0u64;
        let mut unified = true;

        if has_budget_ext {
            let mut budget_props = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
            let mut props2 =
                vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget_props);
            instance.get_physical_device_memory_properties2(pdev, &mut props2);
            for i in 0..heap_count {
                let heap = mem_props.memory_heaps[i];
                if heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                    total_local = total_local.saturating_add(budget_props.heap_budget[i]);
                    let used = budget_props.heap_usage[i];
                    free_budget =
                        free_budget.saturating_add(budget_props.heap_budget[i].saturating_sub(used));
                } else {
                    unified = false;
                }
            }
        } else {
            for i in 0..heap_count {
                let heap = mem_props.memory_heaps[i];
                if heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                    total_local = total_local.saturating_add(heap.size);
                    free_budget = free_budget.saturating_add(heap.size);
                } else {
                    unified = false;
                }
            }
        }

        instance.destroy_instance(None);
        if total_local == 0 {
            return None;
        }
        Some(GpuDeviceProbe {
            backend: "vulkan",
            device_name: name,
            unified_memory: unified,
            total_or_working_set_bytes: total_local,
            free_budget_bytes: free_budget,
            allocated_bytes: total_local.saturating_sub(free_budget),
            max_buffer_bytes: u64::from(props.limits.max_storage_buffer_range),
            simd_width: 32,
            cooperative_matrix: has_coop_mat,
        })
    }
}
