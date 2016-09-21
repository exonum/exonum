use leveldb::management::*;
use leveldb::options::*;
use utils::{open_database,tmpdir};

#[test]
fn test_destroy_database() {
    let tmp = tmpdir("destroy");
    let database = open_database(tmp.path(), true);
    drop(database);
    let options = Options::new();
    let res = destroy(tmp.path(), options);
    assert!(res.is_ok());
}

#[test]
fn test_repair_database() {
    let tmp = tmpdir("repair");
    let database = open_database(tmp.path(), true);
    drop(database);
    let options = Options::new();
    let res = repair(tmp.path(), options);
    assert!(res.is_ok());
}

// Deactivated due do library version dependence
//#[test]
//fn test_destroy_open_database() {
//    let tmp = tmpdir("destroy_open");
//    let database = open_database::<i32>(tmp.path(), true);
//    let options = Options::new();
//    let res = destroy(tmp.path(), options);
//    assert!(res.is_err());
//    drop(database);
//}
