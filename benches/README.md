# Running benchmarks

Criterion is a statistics driven benchmarking library used for benchmarking the backend and the UI.

To run benchmarks, simply run:

```
BENCH_FILE=/path/to/file.aedat4 cargo bench
```

The HTML report can be viewed in `target/criterion/name_of_bench/report/index.html`.

If you want the graphs to be made with GNU Plot, it needs to be installed and available in your `PATH`.

Else, plotters.rs is used to generate the graphs.

## Aggregating results

Criterion can be run multiple times, and the results will be averaged over multiple runs. You can use this to improve the accuracy of the results or even compare speed made to the codebase.

Delete the `target/criterion` directory to clear previous results, especially when switching between different benchmark files.

# Profiling

TODO: setup profiler for CPU/RAM at least, this should be a seperate feature as this reduces the performance of the benchmarks.
