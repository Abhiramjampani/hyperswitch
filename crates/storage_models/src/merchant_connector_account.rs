use common_utils::pii;
use diesel::{AsChangeset, Identifiable, Insertable, Queryable};
use masking::Secret;

use crate::{enums as storage_enums, schema::merchant_connector_account};

#[derive(
    Clone,
    Debug,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Identifiable,
    Queryable,
    router_derive::DebugAsDisplay,
)]
#[diesel(table_name = merchant_connector_account)]
pub struct MerchantConnectorAccount {
    pub id: i32,
    pub merchant_id: String,
    pub connector_name: String,
    pub connector_account_details: serde_json::Value,
    pub test_mode: Option<bool>,
    pub disabled: Option<bool>,
    pub merchant_connector_id: String,
    #[diesel(deserialize_as = super::OptionalDieselArray<serde_json::Value>)]
    pub payment_methods_enabled: Option<Vec<serde_json::Value>>,
    pub connector_type: storage_enums::ConnectorType,
    pub metadata: Option<pii::SecretSerdeValue>,
    pub connector_label: String,
    pub business_country: String,
    pub business_label: String,
    pub business_sub_label: Option<String>,
}

#[derive(Clone, Debug, Default, Insertable, router_derive::DebugAsDisplay)]
#[diesel(table_name = merchant_connector_account)]
pub struct MerchantConnectorAccountNew {
    pub merchant_id: Option<String>,
    pub connector_type: Option<storage_enums::ConnectorType>,
    pub connector_name: Option<String>,
    pub connector_account_details: Option<Secret<serde_json::Value>>,
    pub test_mode: Option<bool>,
    pub disabled: Option<bool>,
    pub merchant_connector_id: String,
    pub payment_methods_enabled: Option<Vec<serde_json::Value>>,
    pub metadata: Option<pii::SecretSerdeValue>,
    pub connector_label: String,
    pub business_country: String,
    pub business_label: String,
    pub business_sub_label: Option<String>,
}

#[derive(Debug)]
pub enum MerchantConnectorAccountUpdate {
    Update {
        merchant_id: Option<String>,
        connector_type: Option<storage_enums::ConnectorType>,
        connector_name: Option<String>,
        connector_account_details: Option<Secret<serde_json::Value>>,
        test_mode: Option<bool>,
        disabled: Option<bool>,
        merchant_connector_id: Option<String>,
        payment_methods_enabled: Option<Vec<serde_json::Value>>,
        metadata: Option<pii::SecretSerdeValue>,
        connector_label: Option<String>,
        business_country: Option<String>,
        business_label: Option<String>,
        business_sub_label: Option<String>,
    },
}
#[derive(Clone, Debug, Default, AsChangeset, router_derive::DebugAsDisplay)]
#[diesel(table_name = merchant_connector_account)]
pub struct MerchantConnectorAccountUpdateInternal {
    merchant_id: Option<String>,
    connector_type: Option<storage_enums::ConnectorType>,
    connector_name: Option<String>,
    connector_account_details: Option<Secret<serde_json::Value>>,
    test_mode: Option<bool>,
    disabled: Option<bool>,
    merchant_connector_id: Option<String>,
    payment_methods_enabled: Option<Vec<serde_json::Value>>,
    metadata: Option<pii::SecretSerdeValue>,
    connector_label: Option<String>,
    business_country: Option<String>,
    business_label: Option<String>,
    business_sub_label: Option<String>,
}

impl From<MerchantConnectorAccountUpdate> for MerchantConnectorAccountUpdateInternal {
    fn from(merchant_connector_account_update: MerchantConnectorAccountUpdate) -> Self {
        match merchant_connector_account_update {
            MerchantConnectorAccountUpdate::Update {
                merchant_id,
                connector_type,
                connector_name,
                connector_account_details,
                test_mode,
                disabled,
                merchant_connector_id,
                payment_methods_enabled,
                metadata,
                connector_label,
                business_country,
                business_label,
                business_sub_label,
            } => Self {
                merchant_id,
                connector_type,
                connector_name,
                connector_account_details,
                test_mode,
                disabled,
                merchant_connector_id,
                payment_methods_enabled,
                metadata,
                connector_label,
                business_label,
                business_country,
                business_sub_label,
            },
        }
    }
}
