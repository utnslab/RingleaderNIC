// Copyright(c) Microsoft Corporation.
// Licensed under the MIT license.

use perftools::timer;

const SAMPLE_SIZE: usize = 2706025;

fn function(depth: usize) {
    timer!("function");

    if depth == 0 {
        return;
    }

    function(depth - 1)
}

fn main() {
    for _ in 0..SAMPLE_SIZE {
        function(8);
    }
    perftools::profiler::write(&mut std::io::stdout(), None).unwrap();
}
