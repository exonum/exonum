use serde::{Serialize, Serializer};

use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::storage::{Map, List, Database, Result as StorageResult};
use exonum::blockchain::Blockchain;
use blockchain_explorer::{TransactionInfo, HexField};

use super::{Role, DigitalRightsTx, DigitalRightsBlockchain, DigitalRightsView, ContentShare, Uuid,
            Fingerprint, Content, Contract, Report, Ownership};

//TODO придумать удобные макросы, чтобы не создавать по 100500 структур с похожими полями

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

#[derive(Debug, Serialize, Clone)]
pub struct ContentShareInfo {
    pub id: u16,
    pub share: u16,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DistributorShortInfo {
    pub id: u16,
    pub name: String
}

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

#[derive(Clone, Debug, Serialize)]
pub struct ContentInfo {
    pub title: String,
    pub fingerprint: HexField<Fingerprint>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub distributors: Vec<u16>,
    pub owners: Vec<ContentShareInfo>,
}

#[derive(Debug, Serialize)]
pub struct ContractInfo {
    pub content: ContentInfo,
    pub plays: u64,
    pub amount: u64,
    pub reports_hash: HexField<Hash>,
    pub reports: Vec<ReportInfo>,
}

#[derive(Debug, Serialize)]
pub struct OwnershipInfo {
    pub content: ContentInfo,
    pub plays: u64,
    pub amount: u64,
    pub reports_hash: HexField<Hash>,
    pub reports: Vec<ReportInfo>,
}

#[derive(Debug, Serialize)]
pub struct ReportInfo {
    pub distributor: DistributorShortInfo,
    pub fingerprint: HexField<Fingerprint>,
    pub time: u64,
    pub plays: u64,
    pub amount: u64,
    pub comment: String,
}

#[derive(Debug, Serialize)]
pub struct DistributorContractInfo {
    pub plays: u64,
    pub amount: u64,
    pub reports_hash: HexField<Hash>,
    pub reports: Vec<ReportInfo>,
}

#[derive(Debug, Serialize)]
pub struct DistributorContentInfo {
    pub title: String,
    pub fingerprint: HexField<Fingerprint>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub distributors: Vec<DistributorShortInfo>,
    pub owners: Vec<ContentShareInfo>,

    pub contract: Option<DistributorContractInfo>,
}

#[derive(Debug, Serialize)]
pub struct OwnerContentInfo {
    pub title: String,
    pub fingerprint: HexField<Fingerprint>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub distributors: Vec<DistributorShortInfo>,
    pub owners: Vec<ContentShareInfo>,

    pub plays: u64,
    pub amount: u64,
    pub reports: Vec<ReportInfo>,
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
            reports: reports,
        }
    }
}

impl OwnershipInfo {
    pub fn new(ownership: Ownership,
               content: ContentInfo,
               reports: Vec<ReportInfo>)
               -> OwnershipInfo {
        OwnershipInfo {
            content: content,
            plays: ownership.plays(),
            amount: ownership.amount(),
            reports_hash: HexField(*ownership.reports_hash()),
            reports: reports,
        }
    }
}

impl ContentInfo {
    pub fn new(fingerprint: Fingerprint,
               content: Content,
               owners: Vec<ContentShareInfo>)
               -> ContentInfo {
        ContentInfo {
            title: content.title().to_string(),
            fingerprint: HexField(fingerprint),
            additional_conditions: content.additional_conditions().to_string(),
            price_per_listen: content.price_per_listen(),
            min_plays: content.min_plays(),
            distributors: content.distributors().into(),
            owners: owners,
        }
    }
}

impl ReportInfo {
    pub fn new(report: Report, distributor: DistributorShortInfo) -> ReportInfo {
        let time = report.time();
        let nsec = (time.sec as u64) * 1_000_000_000 + time.nsec as u64;
        ReportInfo {
            distributor: distributor,
            fingerprint: HexField(*report.fingerprint()),
            time: nsec,
            plays: report.plays(),
            amount: report.amount(),
            comment: report.comment().into(),
        }
    }
}

impl DistributorContractInfo {
    pub fn new(contract: Contract, reports: Vec<ReportInfo>) -> DistributorContractInfo {
        DistributorContractInfo {
            plays: contract.plays(),
            amount: contract.amount(),
            reports_hash: HexField(*contract.reports_hash()),
            reports: reports,
        }
    }
}

impl DistributorContentInfo {
    pub fn new(content: ContentInfo,
               contract: Option<DistributorContractInfo>,
               distributors: Vec<DistributorShortInfo>)
               -> DistributorContentInfo {
        DistributorContentInfo {
            title: content.title,
            fingerprint: content.fingerprint,
            additional_conditions: content.additional_conditions,
            price_per_listen: content.price_per_listen,
            min_plays: content.min_plays,
            distributors: distributors,
            owners: content.owners,

            contract: contract,
        }
    }
}

impl OwnerContentInfo {
    pub fn new(content: ContentInfo,
               ownership: Ownership,
               reports: Vec<ReportInfo>,
               distributors: Vec<DistributorShortInfo>)
               -> OwnerContentInfo {
        OwnerContentInfo {
            title: content.title,
            fingerprint: content.fingerprint,
            additional_conditions: content.additional_conditions,
            price_per_listen: content.price_per_listen,
            min_plays: content.min_plays,
            distributors: distributors,
            owners: content.owners,

            reports: reports,
            plays: ownership.plays(),
            amount: ownership.amount(),
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

    pub fn view(&self) -> DigitalRightsView<D::Fork> {
        self.blockchain.view()
    }

    pub fn participant_id(&self, pub_key: &PublicKey) -> StorageResult<Option<Role>> {
        let view = self.view();
        view.find_participant(pub_key)
    }

    pub fn owner_info(&self, id: u16) -> StorageResult<Option<OwnerInfo>> {
        let view = self.view();
        if let Some(owner) = view.owners().get(id as u64)? {
            let mut ownership = Vec::new();
            for owner_content in view.owner_contents(id).values()? {
                let fingerprint = owner_content.fingerprint().clone();
                let r = view.contents().get(&fingerprint)?.unwrap();
                let owners = self.shares_info(&r.shares())?;
                let content = ContentInfo::new(fingerprint, r, owners);
                let reports = self.find_reports(Role::Owner(id), &fingerprint)?;

                ownership.push(OwnershipInfo::new(owner_content, content, reports));
            }

            let info = OwnerInfo {
                id: id,
                role: "owner",
                name: owner.name().to_string(),
                pub_key: HexField(*owner.pub_key()),
                ownership_hash: HexField(*owner.ownership_hash()),
                ownership: ownership,
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
                let owners = self.shares_info(&r.shares())?;
                let content = ContentInfo::new(fingerprint, r, owners);
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

    pub fn distributor_content_info(&self,
                                    id: u16,
                                    fingerprint: &Fingerprint)
                                    -> StorageResult<Option<DistributorContentInfo>> {
        let v = self.blockchain.view();
        if let Some(content) = v.contents().get(fingerprint)? {
            let owners = self.shares_info(&content.shares())?;
            let content = ContentInfo::new(fingerprint.clone(), content, owners);
            let reports = self.find_reports(Role::Distributor(id), fingerprint)?;
            let contract = v.find_contract(id, fingerprint)?
                .map(|contract| DistributorContractInfo::new(contract.1, reports));
            let distributors = self.distributor_names(&content.distributors)?;

            let info = DistributorContentInfo::new(content, contract, distributors);
            return Ok(Some(info));
        }
        Ok(None)
    }

    pub fn owner_content_info(&self,
                              id: u16,
                              fingerprint: &Fingerprint)
                              -> StorageResult<Option<OwnerContentInfo>> {
        let v = self.blockchain.view();
        if let Some(content) = v.contents().get(fingerprint)? {
            let owners = self.shares_info(&content.shares())?;
            let content = ContentInfo::new(fingerprint.clone(), content, owners);
            let reports = self.find_reports(Role::Owner(id), fingerprint)?;
            let ownership = v.find_ownership(id, fingerprint)?.unwrap().1;
            let distributors = self.distributor_names(&content.distributors)?;

            let info = OwnerContentInfo::new(content, ownership, reports, distributors);
            return Ok(Some(info));
        }
        Ok(None)
    }

    pub fn available_contents(&self, distributor_id: u16) -> StorageResult<Vec<ContentInfo>> {
        let mut v = Vec::new();
        for (fingerprint, content) in self.view().list_content()? {
            if !content.distributors().contains(&distributor_id) {
                let owners = self.shares_info(&content.shares())?;
                v.push(ContentInfo::new(fingerprint, content, owners));
            }
        }
        Ok(v)
    }

    pub fn find_reports(&self,
                        id: Role,
                        fingerprint: &Fingerprint)
                        -> StorageResult<Vec<ReportInfo>> {
        let view = self.view();
        let uuids = match id {
            Role::Owner(id) => {
                view.owner_reports(id, fingerprint)
                    .values()?
            }
            Role::Distributor(id) => {
                view.distributor_reports(id, fingerprint)
                    .values()?
            }
        };

        let mut v = Vec::new();
        for uuid in uuids {
            let report = view.reports().get(&uuid)?.unwrap();
            let id = report.distributor_id();
            let distributor = view.distributors().get(id as u64)?.unwrap();
            let info = DistributorShortInfo {
                id: id,
                name: distributor.name().into()
            };
            v.push(ReportInfo::new(report, info));
        }
        Ok(v)
    }

    pub fn shares_info(&self,
                       content_shares: &Vec<ContentShare>)
                       -> StorageResult<Vec<ContentShareInfo>> {
        let view = self.view();

        let mut r = Vec::new();
        for share in content_shares {
            let owner = view.owners().get(share.owner_id as u64)?.unwrap();
            r.push(ContentShareInfo {
                id: share.owner_id,
                share: share.share,
                name: owner.name().into(),
            })
        }
        Ok(r)
    }

    pub fn distributor_names(&self, ids: &Vec<u16>) -> StorageResult<Vec<DistributorShortInfo>> {
        let view = self.view();

        let mut out = Vec::new();
        for id in ids {
            let distributor = view.distributors().get(*id as u64)?.unwrap();
            out.push(DistributorShortInfo {
                id: *id,
                name: distributor.name().into()
            });
        }
        Ok(out)
    }
}
