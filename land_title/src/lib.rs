#![feature(custom_attribute)]
#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]
#![feature(associated_consts)]

#[macro_use(message, storage_value)]
extern crate exonum;
extern crate serde;
extern crate byteorder;
extern crate blockchain_explorer;
extern crate geo;

mod txs;
mod view;
pub mod cors;
pub mod api;

use exonum::storage::{Map, Database, Error, List};
use exonum::blockchain::{Blockchain};
use exonum::messages::Message;
use exonum::crypto::{Hash, hash};
use std::u64;
use std::ops::Deref;

use geo::{Polygon};
use geo::algorithm::intersects::Intersects;
use geo::algorithm::contains::Contains;

pub use txs::{ObjectTx, TxCreateOwner, TxCreateObject, TxModifyObject,
              TxTransferObject, TxRemoveObject, TxRegister, GeoPoint};
pub use view::{ObjectsView, Owner, Object, User, ObjectId, Ownership};

#[derive(Clone)]
pub struct ObjectsBlockchain<D: Database> {
    pub db: D,
}

impl<D: Database> Deref for ObjectsBlockchain<D> {
    type Target = D;

    fn deref(&self) -> &D {
        &self.db
    }
}

impl<D> Blockchain for ObjectsBlockchain<D> where D: Database
{
    type Database = D;
    type Transaction = ObjectTx;
    type View = ObjectsView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        let mut hashes = Vec::new();
        hashes.extend_from_slice(view.owners().root_hash()?.as_ref());
        hashes.extend_from_slice(view.objects().root_hash()?.as_ref());
        Ok(hash(&hashes))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {

        match *tx {

            ObjectTx::Register(ref tx) => {
                if let Some(user) = view.users().get(&tx.pub_key())? {

                    // TODO: разобраться почему падает нода при возврате ошибки

                    //return Err(Error::new(String::from("User with the same public key already exists.")));

                    //thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Error { message: "Cross titles detected." }', ../src/libcore/result.rs:799
                    // stack backtrace:
                    //    1:        0x10d1f1a48 - std::sys::backtrace::tracing::imp::write::h22f199c1dbb72ba2
                    //    2:        0x10d1f4faf - std::panicking::default_hook::{{closure}}::h9a389c462b6a22dd
                    //    3:        0x10d1f3b2f - std::panicking::default_hook::h852b4223c1c00c59
                    //    4:        0x10d1f4156 - std::panicking::rust_panic_with_hook::hcd9d05f53fa0dafc
                    //    5:        0x10d1f3ff4 - std::panicking::begin_panic::hf6c488cee66e7f17
                    //    6:        0x10d1f3f12 - std::panicking::begin_panic_fmt::hb0a7126ee57cdd27
                    //    7:        0x10d1f3e77 - rust_begin_unwind
                    //    8:        0x10d21fcf0 - core::panicking::panic_fmt::h9af671b78898cdba
                    //    9:        0x10c9dbc0c - core::result::unwrap_failed::h8dac70b2f56ab301
                    //   10:        0x10c9a0570 - <core::result::Result<T, E>>::unwrap::h573785ac8a4426ee
                    //   11:        0x10ca54934 - exonum::node::consensus::<impl exonum::node::NodeHandler<B, S>>::create_block::hdfdfe72f5d4e4cfb
                    //   12:        0x10ca6b743 - exonum::node::consensus::<impl exonum::node::NodeHandler<B, S>>::execute::h8fd0f11a76058c36
                    //   13:        0x10ca69b6b - exonum::node::consensus::<impl exonum::node::NodeHandler<B, S>>::lock::h4d8a3686f95a9263
                    //   14:        0x10ca62979 - exonum::node::consensus::<impl exonum::node::NodeHandler<B, S>>::broadcast_prevote::hea78ec1bd0a0cd0f
                    //   15:        0x10ca664f5 - exonum::node::consensus::<impl exonum::node::NodeHandler<B, S>>::handle_propose_timeout::hdc756ae58413465b
                    //   16:        0x10cb4ef15 - <exonum::node::NodeHandler<B, S> as exonum::events::EventHandler>::handle_timeout::h8a29fc2eb64d94ad
                    //   17:        0x10cace326 - <exonum::events::MioAdapter<H> as mio::handler::Handler>::timeout::hffc3c3de244e84e4
                    //   18:        0x10c98e6fa - <mio::event_loop::EventLoop<H>>::timer_process::h76eb61940f9ec6c0
                    //   19:        0x10c99016d - <mio::event_loop::EventLoop<H>>::run_once::hbc20c3b6730dae79
                    //   20:        0x10c98ebf1 - <mio::event_loop::EventLoop<H>>::run::h4322cf0f5db91766
                    //   21:        0x10cb116e8 - <exonum::events::Events<H> as exonum::events::Reactor<H>>::run::ha7cce77e0bd96a47
                    //   22:        0x10c91d032 - <exonum::node::Node<B>>::run::hf208ca4bbeb05306
                    //   23:        0x10cb746b9 - land_title::run_node::h0a2b799be0145bb2
                    //   24:        0x10cb76715 - land_title::main::h104a68a11d49bb1d
                    //   25:        0x10d1f556a - __rust_maybe_catch_panic
                    //   26:        0x10d1f3616 - std::rt::lang_start::h14cbded5fe3cd915
                    //   27:        0x10cb94809 - main

                    return Ok(());
                }
                let user = User::new(tx.name());
                view.users().put(&tx.pub_key(), user);
            }

            ObjectTx::CreateOwner(ref tx) => {
                let owner = Owner::new(tx.firstname(), tx.lastname(), &hash(&[]));
                view.owners().append(owner)?;
            }

            ObjectTx::CreateObject(ref tx) => {

                let objects = view.objects();

                if let Some(owner) = view.owners().get(tx.owner_id())? {

                    let points = GeoPoint::from_vec(tx.points().to_vec());
                    if points.len() < 3 {
                        //return Err(Error::new(String::from("At least 3 points should be defined.")));
                        return Ok(());
                    }

                    let ls_new = GeoPoint::to_polygon(points);
                    for stored_object in objects.values()? {
                        let stored_points = GeoPoint::from_vec(stored_object.points().to_vec());
                        if ls_new.intersects(&GeoPoint::to_polygon(stored_points)) {
                            //return Err(Error::new(String::from("Cross titles detected.")));
                            return Ok(());
                        }
                    }

                    let object_id = objects.len()? as u64;

                    // update ownership hash
                    let owner_objects = view.owner_objects(tx.owner_id());
                    owner_objects.append(Ownership::new(object_id, true))?;
                    let new_ownership_hash = owner_objects.root_hash()?;
                    let mut owner = view.owners().get(tx.owner_id())?.unwrap();
                    owner.set_ownership_hash(&new_ownership_hash);
                    view.owners().set(tx.owner_id(), owner)?;

                    // update object history hash
                    let object_history = view.object_history(object_id);
                    let hash = hash(&[]);
                    object_history.append(hash)?;
                    let new_history_hash = object_history.root_hash()?;

                    // insert object
                    let object = Object::new(tx.title(), tx.points(), tx.owner_id(), false, &new_history_hash);
                    objects.append(object)?;
                } else {
                    //return Err(Error::new(String::from("Owner not found by id.")));
                    return Ok(());
                }
            }

            ObjectTx::ModifyObject(ref tx) => {

                if let Some(mut object) = view.objects().get(tx.object_id())? {
                    // update object history hash
                    let object_history = view.object_history(tx.object_id());
                    let hash = hash(&[]);
                    object_history.append(hash)?;
                    let new_history_hash = object_history.root_hash()?;

                    // update object
                    object.set_title(tx.title());
                    object.set_points(tx.points());
                    object.set_history_hash(&new_history_hash);
                    view.objects().set(tx.object_id(), object)?;
                }else{
                    //return Err(Error::new(String::from("Object not found by id.")));
                    return Ok(());
                }

            }

            ObjectTx::TransferObject(ref tx) => {

                if let Some(mut object) = view.objects().get(tx.object_id())? {

                        // update ownership hash
                        let old_owner_objects = view.owner_objects(object.owner_id());
                        old_owner_objects.append(Ownership::new(tx.object_id(), false))?;
                        let old_ownership_hash = old_owner_objects.root_hash()?;

                        let new_owner_objects = view.owner_objects(tx.owner_id());
                        new_owner_objects.append(Ownership::new(tx.object_id(), true))?;
                        let new_ownership_hash = new_owner_objects.root_hash()?;

                        // update owners states
                        let mut old_owner = view.owners().get(object.owner_id())?.unwrap();
                        old_owner.set_ownership_hash(&old_ownership_hash);
                        view.owners().set(object.owner_id(), old_owner)?;

                        let mut new_owner = view.owners().get(tx.owner_id())?.unwrap();
                        new_owner.set_ownership_hash(&new_ownership_hash);
                        view.owners().set(tx.owner_id(), new_owner)?;

                        // update object history hash
                        let object_history = view.object_history(tx.object_id());
                        let hash = hash(&[]);
                        object_history.append(hash)?;
                        let new_history_hash = object_history.root_hash()?;

                        // update object
                        object.set_owner(tx.owner_id());
                        object.set_history_hash(&new_history_hash);
                        view.objects().set(tx.object_id(), object)?;

                }else{
                    // return Err(Error::new(String::from("Object not found by id")));
                    return Ok(());
                }

            }

            ObjectTx::RemoveObject(ref tx) => {
                if let Some(mut object) = view.objects().get(tx.object_id())? {

                        // update object history hash
                        let object_history = view.object_history(tx.object_id());
                        let hash = hash(&[]);
                        object_history.append(hash)?;
                        let new_history_hash = object_history.root_hash()?;

                        // update object
                        object.set_deleted(true);
                        object.set_history_hash(&new_history_hash);
                        view.objects().set(tx.object_id(), object)?;

                }else{
                    //return Err(Error::new(String::from("Object not found by id")));
                    return Ok(());
                }
            }

        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::ObjectsBlockchain;
    use view::ObjectsView;
    use txs::{TxCreateOwner, TxCreateObject, TxModifyObject, TxTransferObject, TxRemoveObject, ObjectTx, TxRegister, GeoPoint};
    use exonum::crypto::{gen_keypair};
    use exonum::blockchain::Blockchain;
    use exonum::storage::{Map, List, Error, Database, MemoryDB, Result as StorageResult};

    fn execute_tx<D: Database>(v: &ObjectsView<D::Fork>,
                               tx: ObjectTx)
                               -> StorageResult<()> {
        ObjectsBlockchain::<D>::execute(v, &tx)
    }

    #[test]
    fn register(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();
        let tx_register = TxRegister::new(&p, "test user", &s);

        // Act
        let ok_result = execute_tx::<MemoryDB>(&v, ObjectTx::Register(tx_register.clone()));
        let stored_user = v.users().get(&&p).unwrap().unwrap();
        let err_result = execute_tx::<MemoryDB>(&v, ObjectTx::Register(tx_register.clone()));

        // Assert
        assert_eq!(ok_result.is_ok(), true);
        assert_eq!(err_result.is_ok(), false);
        assert_eq!(stored_user.name(), "test user");
    }

    #[test]
    fn test_add_owner() {

        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();
        let owner_id = 0_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);

        // Act
        let ok_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));
        let stored_owner = v.owners().get(owner_id).unwrap().unwrap();

        // Assert
        assert_eq!(ok_result.is_ok(), true);
        assert_eq!(stored_owner.firstname(), "firstname");
        assert_eq!(stored_owner.lastname(), "lastname");

    }

    #[test]
    fn test_add_object_to_wrong_owner_fails(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let wrong_owner_id = 0_u64;

        let points = vec![1.0, 2.0, 3.0, 4.0];
        let tx_create_object = TxCreateObject::new(&p, "test object title", &points, wrong_owner_id, &s);

        // Act
        let create_object_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object.clone()));

        // Assert
        assert_eq!(create_object_result.is_ok(), false);

    }

    #[test]
    fn test_add_object_with_2_points_fails(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let owner_id = 0_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let create_owner_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));

        let points = vec![1.0, 2.0, 3.0, 4.0];
        let tx_create_object = TxCreateObject::new(&p, "test object title", &points, owner_id, &s);

        // Act
        let create_object_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object.clone()));

        // Assert
        assert_eq!(create_object_result.is_ok(), false);

    }

    #[test]
    fn test_add_object_crossing_another_fails(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let owner_id = 0_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let create_owner_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));

        let points1 = vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0];
        let tx_create_object1 = TxCreateObject::new(&p, "test object title1", &points1, owner_id, &s);

        let points2 = vec![1.0, 1.0, 3.0, 1.0, 3.0, 3.0, 0.0, 3.0];
        let tx_create_object2 = TxCreateObject::new(&p, "test object title2", &points2, owner_id, &s);


        // Act
        let create_object_result1 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object1.clone()));
        let create_object_result2 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object2.clone()));

        // Assert
        assert_eq!(create_object_result1.is_ok(), true);
        assert_eq!(create_object_result2.is_ok(), false);

    }

    #[test]
    fn test_add_object_the_same_to_another_fails(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let owner_id = 0_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let create_owner_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));

        let points1 = vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0];
        let tx_create_object1 = TxCreateObject::new(&p, "test object title1", &points1, owner_id, &s);

        let points2 = vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0];
        let tx_create_object2 = TxCreateObject::new(&p, "test object title2", &points2, owner_id, &s);


        // Act
        let create_object_result1 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object1.clone()));
        let create_object_result2 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object2.clone()));

        // Assert
        assert_eq!(create_object_result1.is_ok(), true);
        assert_eq!(create_object_result2.is_ok(), false);

    }

    #[test]
    fn test_add_object_inside_another_fails(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let owner_id = 0_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let create_owner_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));

        let points1 = vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0];
        let tx_create_object1 = TxCreateObject::new(&p, "test object title1", &points1, owner_id, &s);

        let points2 = vec![0.5, 0.5, 1.5, 0.5, 1.5, 1.5, 0.5, 1.5];
        let tx_create_object2 = TxCreateObject::new(&p, "test object title2", &points2, owner_id, &s);


        // Act
        let create_object_result1 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object1.clone()));
        let create_object_result2 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object2.clone()));

        // Assert
        assert_eq!(create_object_result1.is_ok(), true);
        assert_eq!(create_object_result2.is_ok(), false);

    }

    #[test]
    fn test_add_object(){

        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();

        let owner_id = 0_u64;

        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);

        let object_id1 = 0_u64;
        let points1 = vec![1.0, 1.0, 3.0, 1.0, 3.0, 4.0];
        let tx_create_object1 = TxCreateObject::new(&p, "test object title1", &points1, owner_id, &s);

        let object_id2 = 1_u64;
        let points2 = vec![4.0, 2.0, 5.0, 2.0, 5.0, 4.0];
        let tx_create_object2 = TxCreateObject::new(&p, "test object title2", &points2, owner_id, &s);


        // Act
        let create_owner_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));
        let create_object_result1 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object1.clone()));
        let create_object_result2 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object2.clone()));

        //Assert
        assert_eq!(create_owner_result.is_ok(), true);
        assert_eq!(create_object_result1.is_ok(), true);
        assert_eq!(create_object_result2.is_ok(), true);

        let stored_object1 = v.objects().get(object_id1).unwrap().unwrap();
        let stored_object2 = v.objects().get(object_id2).unwrap().unwrap();
        let stored_owner = v.owners().get(owner_id).unwrap().unwrap();
        let history_hash1 = v.object_history(object_id1).root_hash().unwrap();
        let history_hash2 = v.object_history(object_id2).root_hash().unwrap();
        let ownership_hash = v.owner_objects(owner_id).root_hash().unwrap();
        let objects_for_owner = v.find_objects_for_owner(owner_id).unwrap();

        assert_eq!(stored_object1.title(), "test object title1");
        assert_eq!(stored_object1.owner_id(), owner_id);
        assert_eq!(stored_object1.points(), &[1.0, 1.0, 3.0, 1.0, 3.0, 4.0]);
        assert_eq!(stored_object1.deleted(), false);
        assert_eq!(stored_object1.history_hash(), &history_hash1);

        assert_eq!(stored_object2.title(), "test object title2");
        assert_eq!(stored_object2.owner_id(), owner_id);
        assert_eq!(stored_object2.points(), &[4.0, 2.0, 5.0, 2.0, 5.0, 4.0]);
        assert_eq!(stored_object2.deleted(), false);
        assert_eq!(stored_object2.history_hash(), &history_hash2);

        assert_eq!(stored_owner.ownership_hash(), &ownership_hash);
        assert_eq!(objects_for_owner.len(), 2);
    }

    #[test]
    fn test_tx_modify_object(){

        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let owner_id = 0_u64;
        let object_id = 0_u64;
        let wrong_object_id = 1_u64;
        let points = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tx_create_object = TxCreateObject::new(&p, "test object title", &points, owner_id, &s);
        execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone())).unwrap();
        execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object.clone())).unwrap();
        let modified_title = "modified object title";
        let modified_points = vec![5.0, 6.0, 7.0, 8.0];
        let test_tx_modify_object = TxModifyObject::new(&p, object_id, &modified_title, &modified_points, &s);
        let failed_tx_modify_object = TxModifyObject::new(&p, wrong_object_id, &modified_title, &modified_points, &s);

        // Act
        let ok_modification_result = execute_tx::<MemoryDB>(&v, ObjectTx::ModifyObject(test_tx_modify_object.clone()));
        let err_modification_result = execute_tx::<MemoryDB>(&v, ObjectTx::ModifyObject(failed_tx_modify_object.clone()));
        let modified_object = v.objects().get(object_id).unwrap().unwrap();
        let history_hash = v.object_history(object_id).root_hash().unwrap();

        // Assert
        assert_eq!(ok_modification_result.is_ok(), true);
        assert_eq!(err_modification_result.is_ok(), false);
        assert_eq!(modified_object.title(), "modified object title");
        assert_eq!(modified_object.owner_id(), owner_id);
        assert_eq!(modified_object.points(), &[5.0, 6.0, 7.0, 8.0]);
        assert_eq!(modified_object.deleted(), false);
        assert_eq!(modified_object.history_hash(), &history_hash);
    }

    #[test]
    fn test_tx_transfer_object(){

        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let (p, s) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let owner_id1 = 0_u64;
        let owner_id2 = 1_u64;
        let tx_create_owner = TxCreateOwner::new(&p, "firstname1", "lastname1", &s);
        let tx_create_owner2 = TxCreateOwner::new(&p2, "firstname2", "lastname2", &s2);
        let object_id = 0_u64;
        let wrong_object_id = 1_u64;
        let points = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tx_create_object = TxCreateObject::new(&p, "test object title", &points, owner_id1, &s);
        let create_owner_result1 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone()));
        let create_owner_result2 = execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner2.clone()));
        let create_object_result = execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object.clone()));
        let success_tx_transfer_object = TxTransferObject::new(&p, object_id, owner_id2, &s);
        let failed_tx_transfer_object = TxTransferObject::new(&p, wrong_object_id, owner_id2, &s);

        // Act
        let ok_transfer_result = execute_tx::<MemoryDB>(&v, ObjectTx::TransferObject(success_tx_transfer_object.clone()));
        let err_transfer_result = execute_tx::<MemoryDB>(&v, ObjectTx::TransferObject(failed_tx_transfer_object.clone()));

        let modified_object = v.objects().get(object_id).unwrap().unwrap();
        let modified_owner = v.owners().get(owner_id1).unwrap().unwrap();
        let modified_owner2 = v.owners().get(owner_id2).unwrap().unwrap();
        let history_hash = v.object_history(object_id).root_hash().unwrap();
        let ownership_hash = v.owner_objects(owner_id1).root_hash().unwrap();
        let ownership_hash2 = v.owner_objects(owner_id2).root_hash().unwrap();

        // Assert
        assert_eq!(create_owner_result1.is_ok(), true);
        assert_eq!(create_owner_result2.is_ok(), true);
        assert_eq!(create_object_result.is_ok(), true);

        assert_eq!(ok_transfer_result.is_ok(), true);
        assert_eq!(err_transfer_result.is_ok(), false);
        assert_eq!(modified_object.owner_id(), owner_id2);
        assert_eq!(modified_object.history_hash(), &history_hash);
        assert_eq!(modified_owner.ownership_hash(), &ownership_hash);
        assert_eq!(modified_owner2.ownership_hash(), &ownership_hash2);
    }

    #[test]
    fn test_tx_remove_object(){
        // Arrange
        let b = ObjectsBlockchain { db: MemoryDB::new() };
        let v = b.view();
        let owner_id = 0_u64;
        let (p, s) = gen_keypair();
        let tx_create_owner = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        let object_id = 0_u64;
        let wrong_object_id = 1_u64;
        let points = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tx_create_object = TxCreateObject::new(&p, "test object title", &points, owner_id, &s);
        execute_tx::<MemoryDB>(&v, ObjectTx::CreateOwner(tx_create_owner.clone())).unwrap();
        execute_tx::<MemoryDB>(&v, ObjectTx::CreateObject(tx_create_object.clone())).unwrap();
        let test_tx_remove_object = TxRemoveObject::new(&p, object_id, &s);
        let failed_tx_remove_object = TxRemoveObject::new(&p, wrong_object_id, &s);
        // Act
        let ok_remove_result = execute_tx::<MemoryDB>(&v, ObjectTx::RemoveObject(test_tx_remove_object.clone()));
        let err_remove_result = execute_tx::<MemoryDB>(&v, ObjectTx::RemoveObject(failed_tx_remove_object.clone()));
        let removed_object = v.objects().get(object_id).unwrap().unwrap();
        let history_hash = v.object_history(object_id).root_hash().unwrap();
        // Assert
        assert_eq!(ok_remove_result.is_ok(), true);
        assert_eq!(err_remove_result.is_ok(), false);
        assert_eq!(removed_object.title(), "test object title");
        assert_eq!(removed_object.owner_id(), owner_id);
        assert_eq!(removed_object.points(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(removed_object.deleted(), true);
        assert_eq!(removed_object.history_hash(), &history_hash);

    }
}









