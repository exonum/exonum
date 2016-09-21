#[cfg(test)]
mod compaction {
     use utils::{open_database,tmpdir,db_put_simple};
     use leveldb::compaction::Compaction;

    #[test]
    fn test_iterator_from_to() {
        let tmp = tmpdir("compact");
        let database = &mut open_database(tmp.path(), true);
        db_put_simple(database, b"1", &[1]);
        db_put_simple(database, b"2", &[2]);
        db_put_simple(database, b"3", &[3]);
        db_put_simple(database, b"4", &[4]);
        db_put_simple(database, b"5", &[5]);

        let from = b"2";
        let to = b"4";
        database.compact(from, to);
    }
}
