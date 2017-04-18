// test double attrs

#![feature(plugin, custom_attribute)]
#![plugin(flamer)]
#![flame]

extern crate flame;

#[flame]
fn a() {
    // nothing to do here
}

fn b() {
    a()
}

#[noflame]
fn c() {
    b()
}

#[test]
fn main() {
    c();
    let spans = flame::spans();
    assert_eq!(1, spans.len());
    let roots = &spans[0];
    println!("{:?}",roots);
    // if more than 2 roots, a() was flamed twice or c was flamed
    // main is missing because main isn't closed here
    assert_eq!("b", roots.name);
    assert_eq!(1, roots.children.len());
    assert_eq!("a", roots.children[0].name);
}
