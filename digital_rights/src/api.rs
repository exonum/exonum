use serde::{Serialize, Serializer};

use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::storage::{Map, List, Database, Result as StorageResult};
use exonum::blockchain::Blockchain;
use blockchain_explorer::{TransactionInfo, HexField};

use super::{Role,DigitalRightsTx, DigitalRightsBlockchain, ContentShare, Uuid, Fingerprint, Content, Contract, Report, Ownership};

impl Serialize for DigitalRightsTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        match *self {
            DigitalRightsTx::CreateOwner(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_owner")?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
            }
            DigitalRightsTx::CreateDistributor(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_distributor")?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
            }
            DigitalRightsTx::AddContent(ref tx) => {
                state = ser.serialize_struct("transaction", 8)?;
                ser.serialize_struct_elt(&mut state, "type", "create_distributor")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
                ser.serialize_struct_elt(&mut state, "fingerprint", tx.fingerprint().to_hex())?;
                ser.serialize_struct_elt(&mut state, "title", tx.title())?;
                ser.serialize_struct_elt(&mut state, "price_per_listen", tx.price_per_listen())?;
                ser.serialize_struct_elt(&mut state, "min_plays", tx.min_plays())?;
                ser.serialize_struct_elt(&mut state, "additional_conditions", tx.title())?;
                ser.serialize_struct_elt(&mut state, "owners", tx.owner_shares())?;
            }
            DigitalRightsTx::AddContract(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "add_contract")?;
                ser.serialize_struct_elt(&mut state, "distributor_id", tx.distributor_id())?;
                ser.serialize_struct_elt(&mut state, "fingerprint", tx.fingerprint().to_hex())?;
            }
            DigitalRightsTx::Report(ref tx) => {
                state = ser.serialize_struct("transaction", 8)?;
                ser.serialize_struct_elt(&mut state, "type", "report")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
                ser.serialize_struct_elt(&mut state, "uuid", tx.uuid().to_hex())?;
                ser.serialize_struct_elt(&mut state, "distributor_id", tx.distributor_id())?;
                ser.serialize_struct_elt(&mut state, "fingerprint", tx.fingerprint())?;
                ser.serialize_struct_elt(&mut state, "time", tx.time().sec)?;
                ser.serialize_struct_elt(&mut state, "plays", tx.plays())?;
                ser.serialize_struct_elt(&mut state, "comment", tx.comment())?;
            }
        }
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for DigitalRightsTx {}

#[derive(Debug, Serialize)]
pub struct OwnerInfo {
    pub id: u16,
    pub role: &'static str,
    pub name: String,
    pub pub_key: HexField<PublicKey>,
    pub ownership_hash: HexField<Hash>,
    pub ownership: Vec<OwnershipInfo>,
}

#[derive(Debug, Serialize)]
pub struct DistributorInfo {
    pub id: u16,
    pub role: &'static str,
    pub name: String,
    pub pub_key: HexField<PublicKey>,
    pub available_content: Vec<ContentInfo>,
    pub contracts: Vec<ContractInfo>,
    pub contracts_hash: HexField<Hash>,
}

#[derive(Debug, Serialize)]
pub struct ContentInfo {
    pub title: String,
    pub fingerprint: HexField<Fingerprint>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub distributors: Vec<u16>,
}

#[derive(Debug, Serialize)]
pub struct ContractInfo {
    pub content: ContentInfo,
    pub plays: u64,
    pub amount: u64,
    pub reports_hash: HexField<Hash>,
    pub reports: Vec<ReportInfo>
}

#[derive(Debug, Serialize)]
pub struct OwnershipInfo {
    pub content: ContentInfo,
    pub plays: u64,
    pub amount: u64,
    pub reports_hash: HexField<Hash>,
    pub reports: Vec<ReportInfo>
}

#[derive(Debug, Serialize)]
pub struct ReportInfo {
    pub distributor_id: u16,
    pub fingerprint: HexField<Fingerprint>,
    pub time: u64,
    pub plays: u64,
    pub amount: u64,
    pub comment: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewContent {
    pub title: String,
    pub fingerprint: HexField<Fingerprint>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub owners: Vec<ContentShare>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewReport {
    pub uuid: HexField<Uuid>,
    pub fingerprint: HexField<Fingerprint>,
    pub time: u64,
    pub plays: u64,
    pub comment: String,
}

impl ContractInfo {
    pub fn new(contract: Contract, content: ContentInfo, reports: Vec<ReportInfo>) -> ContractInfo {
        ContractInfo {
            content: content,
            plays: contract.plays(),
            amount: contract.amount(),
            reports_hash: HexField(*contract.reports_hash()),
            reports: reports
        }
    }
}

impl OwnershipInfo {
    pub fn new(ownership: Ownership, content: ContentInfo, reports: Vec<ReportInfo>) -> OwnershipInfo {
        OwnershipInfo {
            content: content,
            plays: ownership.plays(),
            amount: ownership.amount(),
            reports_hash: HexField(*ownership.reports_hash()),
            reports: reports
        }
    }
}


impl ContentInfo {
    pub fn new(fingerprint: Fingerprint, content: Content) -> ContentInfo {
        ContentInfo {
            title: content.title().to_string(),
            fingerprint: HexField(fingerprint),
            additional_conditions: content.additional_conditions().to_string(),
            price_per_listen: content.price_per_listen(),
            min_plays: content.min_plays(),
            distributors: content.distributors().into()
        }
    }
}

impl ReportInfo {
    pub fn new(report: Report) -> ReportInfo {
        let time = report.time();
        let nsec = (time.sec as u64) * 1_000_000_000 + time.nsec as u64;
        ReportInfo {
            distributor_id: report.distributor_id(),
            fingerprint: HexField(*report.fingerprint()),
            time: nsec,
            plays: report.plays(),
            amount: report.amount(),
            comment: report.comment().into(),
        }
    }
}

pub struct DigitalRightsApi<D: Database> {
    blockchain: DigitalRightsBlockchain<D>,
}

impl<D: Database> DigitalRightsApi<D> {
    pub fn new(b: DigitalRightsBlockchain<D>) -> DigitalRightsApi<D> {
        DigitalRightsApi { blockchain: b }
    }

    pub fn participant_id(&self, pub_key: &PublicKey) -> StorageResult<Option<Role>> {
        let view = self.blockchain.view();
        view.find_participant(pub_key)
    }

    pub fn owner_info(&self, id: u16) -> StorageResult<Option<OwnerInfo>> {
        let view = self.blockchain.view();
        if let Some(owner) = view.owners().get(id as u64)? {
            let mut ownership = Vec::new();
            for owner_content in view.owner_contents(id).values()? {
                let fingerprint = owner_content.fingerprint().clone();
                let r = view.contents().get(&fingerprint)?.unwrap();
                let content = ContentInfo::new(fingerprint, r);
                let reports = self.find_reports(Role::Owner(id), &fingerprint)?;

                ownership.push(OwnershipInfo::new(owner_content, content, reports));
            }

            let info = OwnerInfo {
                id: id,
                role: "owner",
                name: owner.name().to_string(),
                pub_key: HexField(*owner.pub_key()),
                ownership_hash: HexField(*owner.ownership_hash()),
                ownership: ownership
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub fn distributor_info(&self, id: u16) -> StorageResult<Option<DistributorInfo>> {
        let view = self.blockchain.view();
        if let Some(distributor) = view.distributors().get(id as u64)? {
            let available_content = self.available_contents(id)?;
            let mut contracts = Vec::new();            
            for contract in view.distributor_contracts(id).values()? {
                let fingerprint = contract.fingerprint().clone();
                let r = view.contents().get(&fingerprint)?.unwrap();
                let content = ContentInfo::new(fingerprint, r);
                let reports = self.find_reports(Role::Distributor(id), &fingerprint)?;

                contracts.push(ContractInfo::new(contract, content, reports));
            }

            let info = DistributorInfo {
                id: id,
                role: "distributor",
                name: distributor.name().to_string(),
                pub_key: HexField(*distributor.pub_key()),
                contracts_hash: HexField(*distributor.contracts_hash()),
                available_content: available_content,
                contracts: contracts,
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub fn available_contents(&self, distributor_id: u16) -> StorageResult<Vec<ContentInfo>> {
        let v = self.blockchain
            .view()
            .list_content()?
            .into_iter()
            .filter(|&(_, ref content)| !content.distributors().contains(&distributor_id))
            .map(|(fingerprint, content)| ContentInfo::new(fingerprint, content))
            .collect();
        Ok(v)
    }

    pub fn find_reports(&self, id: Role, fingerprint: &Fingerprint) -> StorageResult<Vec<ReportInfo>> {
        let view = self.blockchain.view();
        let uuids = match id {
            Role::Owner(id) => {
                view
                    .owner_reports(id, fingerprint)
                    .values()?
            }
            Role::Distributor(id) => {
                view
                    .distributor_reports(id, fingerprint)
                    .values()?
            }
        };

        let mut v = Vec::new();
        for uuid in uuids {
            let report = view.reports().get(&uuid)?.unwrap();
            v.push(ReportInfo::new(report));
        }
        Ok(v)
    }
}
