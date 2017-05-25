extern crate profiler;

use std::fs::File;

pub fn main() {
    profiler::start("update");
        profiler::start("process inputs");
        profiler::end();

        profiler::start("physics");
        for _ in 0..5 {
            profiler::start("broad phase");
            profiler::end();

            profiler::start("narrow phase");
                profiler::start("narrow phase");
                profiler::end();
            profiler::end();
        }
        profiler::end();

        profiler::start("network sync");
        profiler::end();
    profiler::end();

    profiler::start("render");
        profiler::start("build display lists");
        profiler::end();

        profiler::start("draw calls");
        profiler::end();
    profiler::end();

    profiler::dump_html(&mut File::create("out1.html").unwrap()).unwrap();
}
