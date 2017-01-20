extern crate profiler as flame;

use std::fs::File;

pub fn main() {
    flame::start("update");
        flame::start("process inputs");
        flame::end();

        flame::start("physics");
        for i in 0..5 {
            flame::start("broad phase");
            flame::end();

            flame::start("narrow phase");
                flame::start("narrow phase");
                flame::end();
            flame::end();
        }
        flame::end();

        flame::start("network sync");
        flame::end();
    flame::end();

    flame::start("render");
        flame::start("build display lists");
        flame::end();

        flame::start("draw calls");
        flame::end();
    flame::end();

    flame::dump_html(&mut File::create("out1.html").unwrap()).unwrap();
}
