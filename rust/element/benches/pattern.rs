// Micro-benchmarks for allocation patterns: children Vecs and arena Vec.

use criterion::{black_box, Criterion};

fn bench_vec_allocation(c: &mut Criterion) {
    use bumpalo::collections::Vec as BumpVec;

    /// Simulate the pattern of SyntaxNode::children Vecs:
    /// 20k Vecs (one per node), each holding 0-15 NodeIds,
    /// matching the distribution of container vs leaf nodes.
    fn generate_children_sizes() -> Vec<usize> {
        // ~70% leaf (0-2 children), ~20% medium (3-8), ~10% many (9-15)
        let mut sizes = Vec::with_capacity(20_000);
        let mut rng = 42u64;
        for _ in 0..20_000 {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let bucket = (rng >> 32) as u32 % 100;
            let n = if bucket < 70 {
                // leaf — 0..=2
                (rng >> 40) as usize % 3
            } else if bucket < 90 {
                // medium — 3..=8
                3 + (rng >> 40) as usize % 6
            } else {
                // many — 9..=15
                9 + (rng >> 40) as usize % 7
            };
            sizes.push(n);
        }
        sizes
    }

    let sizes = generate_children_sizes();

    // ── global-allocator Vec ──────────────────────────────────────────────
    let mut group = c.benchmark_group("vec_children");
    group.sample_size(100);

    group.bench_function("global_std_vec", |b| {
        b.iter(|| {
            let mut arena: Vec<Vec<usize>> = Vec::with_capacity(sizes.len());
            for &n in &sizes {
                let mut v = Vec::with_capacity(n.min(4));
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    // ── pre-sized global Vec (upper bound capacity) ──────────────────────
    group.bench_function("global_presized", |b| {
        b.iter(|| {
            let mut arena: Vec<Vec<usize>> = Vec::with_capacity(sizes.len());
            for &n in &sizes {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    // ── bump-allocated Vec (bumpalo::collections::Vec) ───────────────────
    // Leak a Bump into a raw pointer so BumpVec borrows can outlive the
    // explicit moves/borrows in the closure.  We reset() the bump between
    // iterations so memory doesn't grow without bound.
    let bump_ptr = Box::into_raw(Box::new(bumpalo::Bump::new()));
    group.bench_function("bump_vec", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let bump_ref: &bumpalo::Bump = bump;
            let mut arena: BumpVec<'_, BumpVec<'_, usize>> = BumpVec::new_in(bump_ref);
            for &n in &sizes {
                let mut v = BumpVec::new_in(bump_ref);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    group.bench_function("bump_vec_presized", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let bump_ref: &bumpalo::Bump = bump;
            let mut arena: BumpVec<'_, BumpVec<'_, usize>> = BumpVec::new_in(bump_ref);
            for &n in &sizes {
                let mut v = BumpVec::with_capacity_in(n, bump_ref);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });
    // Recover and drop the leaked Bump (and all its allocations).
    unsafe {
        drop(Box::from_raw(bump_ptr));
    }

    group.finish();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_hashtag_dispatch(&mut c);
    bench_fixed_width(&mut c);
    bench_vec_allocation(&mut c);
    bench_arena_vec(&mut c);
    c.final_summary();
}

#[cfg(test)]
mod tests {
    // previously had tests
}
