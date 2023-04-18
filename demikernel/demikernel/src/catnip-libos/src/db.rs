
// pub fn testdb(path: String) {
//     // let path = "/tmp/rocksdb";
//     {
//     let db = rocksdb::DB::open_default(path).unwrap();
//     db.put(b"my key", b"my value").unwrap();
//     match db.get(b"my key") {
//         Ok(Some(value)) => println!("retrieved value {}", String::from_utf8(value).unwrap()),
//         Ok(None) => println!("value not found"),
//         Err(e) => println!("operational problem encountered: {}", e),
//     }
//     db.delete(b"my key").unwrap();
//     }
//     let _ = rocksdb::DB::destroy(&rocksdb::Options::default(), path);
// }

pub fn opendb(path: &str) -> std::sync::Arc<rocksdb::DB>{
    // let path = "/tmp/mydb3";
    
    let mut options = rocksdb::Options::default();
    options.set_allow_mmap_reads(true);
    options.set_allow_mmap_writes(true);
    options.increase_parallelism(0);
    let slice = rocksdb::SliceTransform::create_fixed_prefix(8);
    options.set_prefix_extractor(slice);
    let factory_opts = rocksdb::PlainTableFactoryOptions {
        user_key_length: 0,
        bloom_bits_per_key: 10,
        hash_table_ratio: 0.75,
        index_sparseness: 3,
        };
    options.set_plain_table_factory(&factory_opts);
    options.optimize_level_style_compaction(0);
    options.create_if_missing(false);

    let db_ = rocksdb::DB::open(&options, path).unwrap();
    let db_ = std::sync::Arc::new(db_);
    
    println!("Success Open DB");
    db_
}

pub fn gendb(path: &str){
    // let path = "/tmp/mydb3";
    
    let mut options = rocksdb::Options::default();
    options.set_allow_mmap_reads(true);
    options.set_allow_mmap_writes(true);
    options.increase_parallelism(0);
    let slice = rocksdb::SliceTransform::create_fixed_prefix(8);
    options.set_prefix_extractor(slice);
    let factory_opts = rocksdb::PlainTableFactoryOptions {
        user_key_length: 0,
        bloom_bits_per_key: 10,
        hash_table_ratio: 0.75,
        index_sparseness: 3,
        };
    options.set_plain_table_factory(&factory_opts);
    options.optimize_level_style_compaction(0);
    options.create_if_missing(true);

    let db_ = rocksdb::DB::open(&options, path).unwrap();
    for i in 0..5000{
        let k = format!("key{:06}", i);  
        let k1 = key(k.as_bytes());
        let v = format!("value{:06}", i);  
        let v1 = key(v.as_bytes());
        db_.put(k1, v1);
    }
}

pub fn dumpdb(db: std::sync::Arc<rocksdb::DB>){
    let mut options = rocksdb::ReadOptions::default();
    let mut iter = db.iterator( rocksdb::IteratorMode::Start);
    for (key, value) in iter {
        let key_str = std::str::from_utf8(&key).unwrap();
        let value_str = std::str::from_utf8(&value).unwrap();
        println!("Saw {:?} {:?}", key_str, value_str);
    }
}

fn key(k: &[u8]) -> Box<[u8]> {
    k.to_vec().into_boxed_slice()
}

pub fn rocksdb_get(db: std::sync::Arc<rocksdb::DB>, keyint: u16){
    let x = format!("key{:06}", keyint);  
    let b1 = key(x.as_bytes());
    let value =  db.get(b1).unwrap().unwrap();
    
    // let value_str = std::str::from_utf8(&value).unwrap();
    // println!("Get {:?} {:?}", x, value_str);
}

// TODO:: rename if_fin
pub fn rocksdb_scan(db:  std::sync::Arc<rocksdb::DB>, scan_size: u16, pre_location: u16) -> (bool, u16){
    let mut iter:rocksdb::DBIterator;
    if pre_location == 0 {
            iter = db.iterator( rocksdb::IteratorMode::Start);
    }
    else{
        let x = format!("key{:06}", pre_location);  
        let b1 = key(x.as_bytes());
        iter = db.iterator( rocksdb::IteratorMode::From(&b1, rocksdb::Direction::Forward));
    }
    let mut count = 0;
    let mut if_fin = true;
    for (key, value) in iter {
        let key_str = std::str::from_utf8(&key).unwrap();
        let value_str = std::str::from_utf8(&value).unwrap();
        // println!("Saw {:?} {:?}", key_str, value_str);
        count += 1;
        if(count >= scan_size){
            if_fin = false;
            break;
        }
    }
    let next_location = count + pre_location;
    // println!("Start loc: {}, If fin {}, next loc {}", pre_location, if_fin, next_location);
    (if_fin, next_location)
}
