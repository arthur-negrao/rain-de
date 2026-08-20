use std::sync::{Arc, OnceLock};

use divan::{Bencher, black_box};
use rain_utils::finder::{engine::Engine, matcher::Matcher, runner::Runner};

static DATASET: OnceLock<Vec<String>> = OnceLock::new();
static DATASET_ARC: OnceLock<Arc<Vec<String>>> = OnceLock::new();

fn main() {
    // init divan
    divan::main();
}

/// Generates a massive dataset with thousends of Unix-paths with ascii and
/// utf-8 codification.
fn generate_massive_dataset() -> Vec<String> {
    let base_dirs = [
        "/usr/lib/",
        "/etc/",
        "/var/log/",
        "/home/user/.config/",
        "/home/user/.local/share/",
        "/opt/",
        "/home/xayzen/",
    ];

    let projects = [
        "rain-dock",
        "rain-launcher",
        "hyprland",
        "waybar",
        "yazi",
        "zabbix-agent",
        "sequence-simulator",
        "linux-kernel",
        "xwayland",
    ];

    let files = [
        "main.rs",
        "config.json",
        "app.ts",
        "engine.rs",
        "daemon.cpp",
        "fuzzy.rs",
        "Match_Ação.rs", // utf-8
        "util.h",
        "index.js",
    ];

    let mut dataset = Vec::with_capacity(500_000);

    for i in 0..1000 {
        for dir in base_dirs.iter() {
            for proj in projects.iter() {
                for file in files.iter() {
                    // gen deepth paths
                    let path =
                        format!("{}{}/module_{}/src/{}", dir, proj, i, file);
                    dataset.push(path);
                }
            }
        }
    }

    dataset
}

fn get_dataset() -> &'static [String] {
    DATASET.get_or_init(|| generate_massive_dataset())
}

fn get_dataset_arc() -> Arc<Vec<String>> {
    DATASET_ARC
        .get_or_init(|| Arc::new(generate_massive_dataset()))
        .clone()
}

fn get_refs() -> Vec<(&'static str, usize)> {
    get_dataset()
        .iter()
        .enumerate()
        .map(|(id, s)| (s.as_ref(), id))
        .collect()
}

mod matcher {
    use super::*;

    #[divan::bench]
    fn common_ascii_pattern(bencher: Bencher) {
        let refs = get_refs();

        let pattern: Vec<char> = "rain".chars().collect();
        let is_ascii = pattern.iter().all(|c| c.is_ascii());
        let pattern_bytes: Vec<u8> = if is_ascii {
            pattern
                .iter()
                .map(|c| c.to_ascii_lowercase() as u8)
                .collect()
        } else {
            Vec::new()
        };

        bencher
            .with_inputs(|| Matcher::new())
            .bench_refs(|matcher| {
                let results = matcher.rank_and_sort(
                    black_box(&refs),
                    black_box(&pattern),
                    black_box(&pattern_bytes),
                    is_ascii,
                );

                black_box(results);
            });
    }

    #[divan::bench]
    fn rare_pattern(bencher: Bencher) {
        let refs = get_refs();

        let pattern: Vec<char> = "xyz".chars().collect();
        let is_ascii = pattern.iter().all(|c| c.is_ascii());
        let pattern_bytes: Vec<u8> = if is_ascii {
            pattern
                .iter()
                .map(|c| c.to_ascii_lowercase() as u8)
                .collect()
        } else {
            Vec::new()
        };

        bencher
            .with_inputs(|| Matcher::new())
            .bench_refs(|matcher| {
                let results = matcher.rank_and_sort(
                    black_box(&refs),
                    black_box(&pattern),
                    black_box(&pattern_bytes),
                    is_ascii,
                );

                black_box(results);
            });
    }
}

mod sync_engine {
    use super::*;

    #[divan::bench]
    fn one_shot_rare(bencher: Bencher) {
        let refs: Vec<(&str, usize)> = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "xyz";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn one_shot_common(bencher: Bencher) {
        let refs: Vec<(&str, usize)> = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "rain";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn cache_hit(bencher: Bencher) {
        let refs: Vec<(&str, usize)> = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "rai";
                let _ = engine.search(black_box(pattern), false);

                let pattern = "rain";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn cache_invalidation(bencher: Bencher) {
        let refs = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "rain";
                let _ = engine.search(black_box(pattern), false);

                let pattern = "rai";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn cache_hit_rare(bencher: Bencher) {
        let refs = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "xy";
                let _ = engine.search(black_box(pattern), false);

                let pattern = "xyz";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn cache_invalidation_rare(bencher: Bencher) {
        let refs = get_refs();

        bencher
            .with_inputs(|| Engine::new(black_box(&refs)))
            .bench_refs(|engine| {
                let pattern = "xyz";
                let _ = engine.search(black_box(pattern), false);

                let pattern = "xy";
                let results = engine.search(black_box(pattern), false);

                black_box(results);
            });
    }

    #[divan::bench]
    fn unicode_utf8_path(bencher: Bencher) {
        let refs = get_refs();

        bencher
            .with_inputs(|| Engine::new(&refs))
            .bench_refs(|engine| {
                let results = engine.search(black_box("ação"), false);
                black_box(results);
            });
    }

    #[divan::bench]
    fn with_sorting_overhead(bencher: Bencher) {
        let refs = get_refs();

        bencher
            .with_inputs(|| Engine::new(&refs))
            .bench_refs(|engine| {
                // sort algorithm overhead
                let results = engine.search(black_box("rain"), true);
                black_box(results);
            });
    }
}

mod async_engine {
    use super::*;

    #[cfg(not(feature = "tokio"))]
    mod blocking {
        use super::*;

        #[divan::bench]
        fn one_shot_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern = "rain";
                    let result = dispatcher
                        .submit_blocking(pattern, false)
                        .unwrap();

                    black_box(result);
                });
        }

        #[divan::bench]
        fn one_shot_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern = "xyz";
                    let result = dispatcher
                        .submit_blocking(pattern, false)
                        .unwrap();
                    black_box(result);
                });
        }

        #[divan::bench]
        fn cache_hit_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern1 = "rai";
                    let result1 = dispatcher
                        .submit_blocking(pattern1, false)
                        .unwrap();

                    let pattern2 = "rain";
                    let result2 = dispatcher
                        .submit_blocking(pattern2, false)
                        .unwrap();

                    black_box((result1, result2));
                });
        }

        #[divan::bench]
        fn cache_hit_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern1 = "xy";
                    let result1 = dispatcher
                        .submit_blocking(pattern1, false)
                        .unwrap();

                    let pattern2 = "xyz";
                    let result2 = dispatcher
                        .submit_blocking(pattern2, false)
                        .unwrap();

                    black_box((result1, result2));
                });
        }

        #[divan::bench]
        fn cache_invalidation_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern1 = "rain";
                    let result1 = dispatcher
                        .submit_blocking(pattern1, false)
                        .unwrap();

                    let pattern2 = "rai";
                    let result2 = dispatcher
                        .submit_blocking(pattern2, false)
                        .unwrap();

                    black_box((result1, result2));
                });
        }

        #[divan::bench]
        fn cache_invalidation_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    let pattern1 = "xyz";
                    let result1 = dispatcher
                        .submit_blocking(pattern1, false)
                        .unwrap();

                    let pattern2 = "xy";
                    let result2 = dispatcher
                        .submit_blocking(pattern2, false)
                        .unwrap();

                    black_box((result1, result2));
                });
        }
    }

    #[cfg(feature = "tokio")]
    mod tokio_runtime {
        use super::*;

        #[divan::bench]
        fn one_shot_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern = "rain";
                        let result =
                            dispatcher.submit(pattern, false).await.unwrap();

                        black_box(result);
                    });
                });
        }

        #[divan::bench]
        fn one_shot_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern = "xyz";
                        let result =
                            dispatcher.submit(pattern, false).await.unwrap();

                        black_box(result);
                    });
                });
        }

        #[divan::bench]
        fn cache_hit_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern1 = "rai";
                        let result1 =
                            dispatcher.submit(pattern1, false).await.unwrap();

                        let pattern2 = "rain";
                        let result2 =
                            dispatcher.submit(pattern2, false).await.unwrap();

                        black_box((result1, result2));
                    });
                });
        }

        #[divan::bench]
        fn cache_hit_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern1 = "xy";
                        let result1 =
                            dispatcher.submit(pattern1, false).await.unwrap();

                        let pattern2 = "xyz";
                        let result2 =
                            dispatcher.submit(pattern2, false).await.unwrap();

                        black_box((result1, result2));
                    });
                });
        }

        #[divan::bench]
        fn cache_invalidation_common(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern1 = "rain";
                        let result1 =
                            dispatcher.submit(pattern1, false).await.unwrap();

                        let pattern2 = "rai";
                        let result2 =
                            dispatcher.submit(pattern2, false).await.unwrap();

                        black_box((result1, result2));
                    });
                });
        }

        #[divan::bench]
        fn cache_invalidation_rare(bencher: Bencher) {
            let dataset = get_dataset_arc();

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fail to init Tokio runtime.");

            let _guard = rt.enter();

            bencher
                .with_inputs(|| {
                    Runner::new(dataset.clone(), 8).expect("Engine init failed")
                })
                .bench_refs(|runner| {
                    let dispatcher = runner.dispatcher();

                    rt.block_on(async {
                        let pattern1 = "xyz";
                        let result1 =
                            dispatcher.submit(pattern1, false).await.unwrap();

                        let pattern2 = "xy";
                        let result2 =
                            dispatcher.submit(pattern2, false).await.unwrap();

                        black_box((result1, result2));
                    });
                });
        }
    }
}
