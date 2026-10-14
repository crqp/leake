mod common;

use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::cell::Cell;

use common::noisy_frame;
use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;

thread_local! {
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
        let _ = ALLOCATED.try_with(|count| count.set(count.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: AllocLayout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: AllocLayout) -> *mut u8 {
        let _ = ALLOCATED.try_with(|count| count.set(count.get() + layout.size()));
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: AllocLayout, new_size: usize) -> *mut u8 {
        let _ = ALLOCATED.try_with(|count| count.set(count.get() + new_size));
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocated_bytes(action: impl FnOnce()) -> usize {
    let before = ALLOCATED.with(Cell::get);
    action();
    ALLOCATED.with(Cell::get) - before
}

#[test]
fn packed_formats_allocate_a_fixed_amount_per_pixel() {
    assert_allocation(AllocationCase {
        format: VtfFormat::Rgb565,
        quality: Quality::Slow,
        dither: true,
        small: 64,
        large: 128,
        budget: PACKED_64_BUDGET,
        slope_limit: PACKED_SLOPE,
    });
}

#[test]
fn dxt_formats_allocate_a_fixed_amount_per_block() {
    assert_allocation(AllocationCase {
        format: VtfFormat::Dxt1,
        quality: Quality::Slow,
        dither: true,
        small: 64,
        large: 128,
        budget: DXT1_64_BUDGET,
        slope_limit: DXT1_SLOPE,
    });
    assert_allocation(AllocationCase {
        format: VtfFormat::Dxt5,
        quality: Quality::Slow,
        dither: true,
        small: 64,
        large: 128,
        budget: DXT5_64_BUDGET,
        slope_limit: DXT5_SLOPE,
    });
    assert_allocation(AllocationCase {
        format: VtfFormat::Dxt1,
        quality: Quality::VerySlow,
        dither: true,
        small: 32,
        large: 64,
        budget: DXT1_VERY_SLOW_BUDGET,
        slope_limit: DXT1_VERY_SLOW_SLOPE,
    });
    assert_allocation(AllocationCase {
        format: VtfFormat::Dxt5,
        quality: Quality::VerySlow,
        dither: true,
        small: 32,
        large: 64,
        budget: DXT5_VERY_SLOW_BUDGET,
        slope_limit: DXT5_VERY_SLOW_SLOPE,
    });
}

fn assert_allocation(case: AllocationCase) {
    let small = measure(&case, case.small);
    let large = measure(&case, case.large);
    let extra_blocks = (blocks(case.large) - blocks(case.small)) as usize;
    let slope = (large - small) / extra_blocks;
    println!(
        "allocation {:<8} {:<9} {}x{}={} bytes {}x{}={} bytes slope={} bytes/block",
        case.format,
        case.quality,
        case.small,
        case.small,
        small,
        case.large,
        case.large,
        large,
        slope,
    );
    assert!(
        small <= case.budget,
        "{} allocated {small} bytes at {}x{}, budget {}",
        case.format,
        case.small,
        case.small,
        case.budget
    );
    assert!(
        slope <= case.slope_limit,
        "{} allocated {slope} bytes per extra block, slope limit {}",
        case.format,
        case.slope_limit
    );
}

fn measure(case: &AllocationCase, size: u32) -> usize {
    let raster = noisy_frame(size, size);
    for _ in 0..WARMUPS {
        encode_raster(&raster, case);
    }
    let mut smallest = usize::MAX;
    for _ in 0..MEASUREMENTS {
        let measured = allocated_bytes(|| encode_raster(&raster, case));
        smallest = smallest.min(measured);
    }
    smallest
}

fn encode_raster(raster: &Frame, case: &AllocationCase) {
    let _ = encode::encode(raster, case.format, case.quality, case.dither, 1, None);
}

fn blocks(size: u32) -> u32 {
    (size / 4) * (size / 4)
}

#[derive(Clone, Copy)]
struct AllocationCase {
    format: VtfFormat,
    quality: Quality,
    dither: bool,
    small: u32,
    large: u32,
    budget: usize,
    slope_limit: usize,
}

const WARMUPS: usize = 3;

const MEASUREMENTS: usize = 3;

const PACKED_64_BUDGET: usize = 20_000;

const PACKED_SLOPE: usize = 64;

const DXT1_64_BUDGET: usize = 8_192;

const DXT1_SLOPE: usize = 64;

const DXT5_64_BUDGET: usize = 12_288;

const DXT5_SLOPE: usize = 128;

const DXT1_VERY_SLOW_BUDGET: usize = 8_192;

const DXT1_VERY_SLOW_SLOPE: usize = 64;

const DXT5_VERY_SLOW_BUDGET: usize = 12_288;

const DXT5_VERY_SLOW_SLOPE: usize = 128;
