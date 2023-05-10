use std::collections::HashMap;

use api_models::payments::{self, OrderDetails};
use base64::Engine;
use common_utils::{
    date_time,
    errors::ReportSwitchExt,
    pii::{self, Email},
};
use error_stack::{report, IntoReport, ResultExt};
use masking::Secret;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serializer;

use crate::{
    consts,
    core::errors::{self, CustomResult},
    pii::PeekInterface,
    types::{self, api, PaymentsCancelData, ResponseId},
    utils::{self, OptionExt, ValueExt},
};

pub fn missing_field_err(
    message: &'static str,
) -> Box<dyn Fn() -> error_stack::Report<errors::ConnectorError> + '_> {
    Box::new(move || {
        errors::ConnectorError::MissingRequiredField {
            field_name: message,
        }
        .into()
    })
}

type Error = error_stack::Report<errors::ConnectorError>;

pub trait AccessTokenRequestInfo {
    fn get_request_id(&self) -> Result<String, Error>;
}

impl AccessTokenRequestInfo for types::RefreshTokenRouterData {
    fn get_request_id(&self) -> Result<String, Error> {
        self.request
            .id
            .clone()
            .ok_or_else(missing_field_err("request.id"))
    }
}

pub trait RouterData {
    fn get_billing(&self) -> Result<&api::Address, Error>;
    fn get_billing_country(&self) -> Result<api_models::enums::CountryAlpha2, Error>;
    fn get_billing_phone(&self) -> Result<&api::PhoneDetails, Error>;
    fn get_description(&self) -> Result<String, Error>;
    fn get_return_url(&self) -> Result<String, Error>;
    fn get_billing_address(&self) -> Result<&api::AddressDetails, Error>;
    fn get_shipping_address(&self) -> Result<&api::AddressDetails, Error>;
    fn get_connector_meta(&self) -> Result<pii::SecretSerdeValue, Error>;
    fn get_session_token(&self) -> Result<String, Error>;
    fn to_connector_meta<T>(&self) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned;
    fn is_three_ds(&self) -> bool;
    fn get_payment_method_token(&self) -> Result<String, Error>;
    fn get_customer_id(&self) -> Result<String, Error>;
    fn get_connector_customer_id(&self) -> Result<String, Error>;
}

impl<Flow, Request, Response> RouterData for types::RouterData<Flow, Request, Response> {
    fn get_billing(&self) -> Result<&api::Address, Error> {
        self.address
            .billing
            .as_ref()
            .ok_or_else(missing_field_err("billing"))
    }

    fn get_billing_country(&self) -> Result<api_models::enums::CountryAlpha2, Error> {
        self.address
            .billing
            .as_ref()
            .and_then(|a| a.address.as_ref())
            .and_then(|ad| ad.country)
            .ok_or_else(missing_field_err("billing.address.country"))
    }

    fn get_billing_phone(&self) -> Result<&api::PhoneDetails, Error> {
        self.address
            .billing
            .as_ref()
            .and_then(|a| a.phone.as_ref())
            .ok_or_else(missing_field_err("billing.phone"))
    }
    fn get_description(&self) -> Result<String, Error> {
        self.description
            .clone()
            .ok_or_else(missing_field_err("description"))
    }
    fn get_return_url(&self) -> Result<String, Error> {
        self.return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
    fn get_billing_address(&self) -> Result<&api::AddressDetails, Error> {
        self.address
            .billing
            .as_ref()
            .and_then(|a| a.address.as_ref())
            .ok_or_else(missing_field_err("billing.address"))
    }
    fn get_connector_meta(&self) -> Result<pii::SecretSerdeValue, Error> {
        self.connector_meta_data
            .clone()
            .ok_or_else(missing_field_err("connector_meta_data"))
    }

    fn get_session_token(&self) -> Result<String, Error> {
        self.session_token
            .clone()
            .ok_or_else(missing_field_err("session_token"))
    }

    fn to_connector_meta<T>(&self) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        self.get_connector_meta()?
            .parse_value(std::any::type_name::<T>())
            .change_context(errors::ConnectorError::NoConnectorMetaData)
    }

    fn is_three_ds(&self) -> bool {
        matches!(
            self.auth_type,
            storage_models::enums::AuthenticationType::ThreeDs
        )
    }

    fn get_shipping_address(&self) -> Result<&api::AddressDetails, Error> {
        self.address
            .shipping
            .as_ref()
            .and_then(|a| a.address.as_ref())
            .ok_or_else(missing_field_err("shipping.address"))
    }
    fn get_payment_method_token(&self) -> Result<String, Error> {
        self.payment_method_token
            .clone()
            .ok_or_else(missing_field_err("payment_method_token"))
    }
    fn get_customer_id(&self) -> Result<String, Error> {
        self.customer_id
            .to_owned()
            .ok_or_else(missing_field_err("customer_id"))
    }
    fn get_connector_customer_id(&self) -> Result<String, Error> {
        self.connector_customer
            .to_owned()
            .ok_or_else(missing_field_err("connector_customer_id"))
    }
}

pub trait PaymentsAuthorizeRequestData {
    fn is_auto_capture(&self) -> Result<bool, Error>;
    fn get_email(&self) -> Result<Email, Error>;
    fn get_browser_info(&self) -> Result<types::BrowserInformation, Error>;
    fn get_order_details(&self) -> Result<Vec<OrderDetails>, Error>;
    fn get_card(&self) -> Result<api::Card, Error>;
    fn get_return_url(&self) -> Result<String, Error>;
    fn connector_mandate_id(&self) -> Option<String>;
    fn is_mandate_payment(&self) -> bool;
    fn get_webhook_url(&self) -> Result<String, Error>;
    fn get_router_return_url(&self) -> Result<String, Error>;
}

impl PaymentsAuthorizeRequestData for types::PaymentsAuthorizeData {
    fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(storage_models::enums::CaptureMethod::Automatic) | None => Ok(true),
            Some(storage_models::enums::CaptureMethod::Manual) => Ok(false),
            Some(_) => Err(errors::ConnectorError::CaptureMethodNotSupported.into()),
        }
    }
    fn get_email(&self) -> Result<Email, Error> {
        self.email.clone().ok_or_else(missing_field_err("email"))
    }
    fn get_browser_info(&self) -> Result<types::BrowserInformation, Error> {
        self.browser_info
            .clone()
            .ok_or_else(missing_field_err("browser_info"))
    }
    fn get_order_details(&self) -> Result<Vec<OrderDetails>, Error> {
        self.order_details
            .clone()
            .ok_or_else(missing_field_err("order_details"))
    }

    fn get_card(&self) -> Result<api::Card, Error> {
        match self.payment_method_data.clone() {
            api::PaymentMethodData::Card(card) => Ok(card),
            _ => Err(missing_field_err("card")()),
        }
    }
    fn get_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
    fn connector_mandate_id(&self) -> Option<String> {
        self.mandate_id
            .as_ref()
            .and_then(|mandate_ids| match &mandate_ids.mandate_reference_id {
                Some(api_models::payments::MandateReferenceId::ConnectorMandateId(
                    connector_mandate_ids,
                )) => connector_mandate_ids.connector_mandate_id.clone(),
                _ => None,
            })
    }
    fn is_mandate_payment(&self) -> bool {
        self.setup_mandate_details.is_some()
            || self
                .mandate_id
                .as_ref()
                .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
                .is_some()
    }
    fn get_webhook_url(&self) -> Result<String, Error> {
        self.webhook_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
    fn get_router_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
}

pub trait BrowserInformationData {
    fn get_ip_address(&self) -> Result<std::net::IpAddr, Error>;
}

impl BrowserInformationData for types::BrowserInformation {
    fn get_ip_address(&self) -> Result<std::net::IpAddr, Error> {
        self.ip_address.ok_or_else(missing_field_err("ip_address"))
    }
}

pub trait PaymentsCompleteAuthorizeRequestData {
    fn is_auto_capture(&self) -> Result<bool, Error>;
}

impl PaymentsCompleteAuthorizeRequestData for types::CompleteAuthorizeData {
    fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(storage_models::enums::CaptureMethod::Automatic) | None => Ok(true),
            Some(storage_models::enums::CaptureMethod::Manual) => Ok(false),
            Some(_) => Err(errors::ConnectorError::CaptureMethodNotSupported.into()),
        }
    }
}

pub trait PaymentsSyncRequestData {
    fn is_auto_capture(&self) -> Result<bool, Error>;
    fn get_connector_transaction_id(&self) -> CustomResult<String, errors::ConnectorError>;
}

impl PaymentsSyncRequestData for types::PaymentsSyncData {
    fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(storage_models::enums::CaptureMethod::Automatic) | None => Ok(true),
            Some(storage_models::enums::CaptureMethod::Manual) => Ok(false),
            Some(_) => Err(errors::ConnectorError::CaptureMethodNotSupported.into()),
        }
    }
    fn get_connector_transaction_id(&self) -> CustomResult<String, errors::ConnectorError> {
        match self.connector_transaction_id.clone() {
            ResponseId::ConnectorTransactionId(txn_id) => Ok(txn_id),
            _ => Err(errors::ValidationError::IncorrectValueProvided {
                field_name: "connector_transaction_id",
            })
            .into_report()
            .attach_printable("Expected connector transaction ID not found")
            .change_context(errors::ConnectorError::MissingConnectorTransactionID)?,
        }
    }
}

pub trait PaymentsCancelRequestData {
    fn get_amount(&self) -> Result<i64, Error>;
    fn get_currency(&self) -> Result<storage_models::enums::Currency, Error>;
    fn get_cancellation_reason(&self) -> Result<String, Error>;
}

impl PaymentsCancelRequestData for PaymentsCancelData {
    fn get_amount(&self) -> Result<i64, Error> {
        self.amount.ok_or_else(missing_field_err("amount"))
    }
    fn get_currency(&self) -> Result<storage_models::enums::Currency, Error> {
        self.currency.ok_or_else(missing_field_err("currency"))
    }
    fn get_cancellation_reason(&self) -> Result<String, Error> {
        self.cancellation_reason
            .clone()
            .ok_or_else(missing_field_err("cancellation_reason"))
    }
}

pub trait RefundsRequestData {
    fn get_connector_refund_id(&self) -> Result<String, Error>;
}

impl RefundsRequestData for types::RefundsData {
    #[track_caller]
    fn get_connector_refund_id(&self) -> Result<String, Error> {
        self.connector_refund_id
            .clone()
            .get_required_value("connector_refund_id")
            .change_context(errors::ConnectorError::MissingConnectorTransactionID)
    }
}

static CARD_REGEX: Lazy<HashMap<CardIssuer, Result<Regex, regex::Error>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    // Reference: https://gist.github.com/michaelkeevildown/9096cd3aac9029c4e6e05588448a8841
    // [#379]: Determine card issuer from card BIN number
    map.insert(CardIssuer::Master, Regex::new(r"^5[1-5][0-9]{14}$"));
    map.insert(CardIssuer::AmericanExpress, Regex::new(r"^3[47][0-9]{13}$"));
    map.insert(CardIssuer::Visa, Regex::new(r"^4[0-9]{12}(?:[0-9]{3})?$"));
    map.insert(CardIssuer::Discover, Regex::new(r"^65[4-9][0-9]{13}|64[4-9][0-9]{13}|6011[0-9]{12}|(622(?:12[6-9]|1[3-9][0-9]|[2-8][0-9][0-9]|9[01][0-9]|92[0-5])[0-9]{10})$"));
    map.insert(
        CardIssuer::Maestro,
        Regex::new(r"^(5018|5020|5038|5893|6304|6759|6761|6762|6763)[0-9]{8,15}$"),
    );
    map.insert(
        CardIssuer::DinersClub,
        Regex::new(r"^3(?:0[0-5]|[68][0-9])[0-9]{11}$"),
    );
    map.insert(
        CardIssuer::JCB,
        Regex::new(r"^(3(?:088|096|112|158|337|5(?:2[89]|[3-8][0-9]))\d{12})$"),
    );
    map
});

#[derive(Debug, Copy, Clone, strum::Display, Eq, Hash, PartialEq)]
pub enum CardIssuer {
    AmericanExpress,
    Master,
    Maestro,
    Visa,
    Discover,
    DinersClub,
    JCB,
}

pub trait CardData {
    fn get_card_expiry_year_2_digit(&self) -> Secret<String>;
    fn get_card_issuer(&self) -> Result<CardIssuer, Error>;
    fn get_card_expiry_month_year_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Secret<String>;
    fn get_expiry_date_as_yyyymm(&self, delimiter: &str) -> Secret<String>;
}

impl CardData for api::Card {
    fn get_card_expiry_year_2_digit(&self) -> Secret<String> {
        let binding = self.card_exp_year.clone();
        let year = binding.peek();
        Secret::new(year[year.len() - 2..].to_string())
    }
    fn get_card_issuer(&self) -> Result<CardIssuer, Error> {
        get_card_issuer(self.card_number.peek())
    }
    fn get_card_expiry_month_year_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Secret<String> {
        let year = self.get_card_expiry_year_2_digit();
        Secret::new(format!(
            "{}{}{}",
            self.card_exp_month.peek().clone(),
            delimiter,
            year.peek()
        ))
    }
    fn get_expiry_date_as_yyyymm(&self, delimiter: &str) -> Secret<String> {
        let mut x = self.card_exp_year.peek().clone();
        if x.len() == 2 {
            x = format!("20{}", x);
        }
        Secret::new(format!(
            "{}{}{}",
            x,
            delimiter,
            self.card_exp_month.peek().clone()
        ))
    }
}

#[track_caller]
fn get_card_issuer(card_number: &str) -> Result<CardIssuer, Error> {
    for (k, v) in CARD_REGEX.iter() {
        let regex: Regex = v
            .clone()
            .into_report()
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;
        if regex.is_match(card_number) {
            return Ok(*k);
        }
    }
    Err(error_stack::Report::new(
        errors::ConnectorError::NotImplemented("Card Type".into()),
    ))
}
pub trait WalletData {
    fn get_wallet_token(&self) -> Result<String, Error>;
    fn get_wallet_token_as_json<T>(&self) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned;
}

impl WalletData for api::WalletData {
    fn get_wallet_token(&self) -> Result<String, Error> {
        match self {
            Self::GooglePay(data) => Ok(data.tokenization_data.token.clone()),
            Self::ApplePay(data) => Ok(data.get_applepay_decoded_payment_data()?),
            Self::PaypalSdk(data) => Ok(data.token.clone()),
            _ => Err(errors::ConnectorError::InvalidWallet.into()),
        }
    }
    fn get_wallet_token_as_json<T>(&self) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_str::<T>(&self.get_wallet_token()?)
            .into_report()
            .change_context(errors::ConnectorError::InvalidWalletToken)
    }
}

pub trait ApplePay {
    fn get_applepay_decoded_payment_data(&self) -> Result<String, Error>;
}

impl ApplePay for payments::ApplePayWalletData {
    fn get_applepay_decoded_payment_data(&self) -> Result<String, Error> {
        let token = String::from_utf8(
            consts::BASE64_ENGINE
                .decode(&self.payment_data)
                .into_report()
                .change_context(errors::ConnectorError::InvalidWalletToken)?,
        )
        .into_report()
        .change_context(errors::ConnectorError::InvalidWalletToken)?;
        Ok(token)
    }
}
pub trait PhoneDetailsData {
    fn get_number(&self) -> Result<Secret<String>, Error>;
    fn get_country_code(&self) -> Result<String, Error>;
}

impl PhoneDetailsData for api::PhoneDetails {
    fn get_country_code(&self) -> Result<String, Error> {
        self.country_code
            .clone()
            .ok_or_else(missing_field_err("billing.phone.country_code"))
    }
    fn get_number(&self) -> Result<Secret<String>, Error> {
        self.number
            .clone()
            .ok_or_else(missing_field_err("billing.phone.number"))
    }
}

pub trait AddressDetailsData {
    fn get_first_name(&self) -> Result<&Secret<String>, Error>;
    fn get_last_name(&self) -> Result<&Secret<String>, Error>;
    fn get_line1(&self) -> Result<&Secret<String>, Error>;
    fn get_city(&self) -> Result<&String, Error>;
    fn get_line2(&self) -> Result<&Secret<String>, Error>;
    fn get_zip(&self) -> Result<&Secret<String>, Error>;
    fn get_country(&self) -> Result<&api_models::enums::CountryAlpha2, Error>;
    fn get_combined_address_line(&self) -> Result<Secret<String>, Error>;
}

impl AddressDetailsData for api::AddressDetails {
    fn get_first_name(&self) -> Result<&Secret<String>, Error> {
        self.first_name
            .as_ref()
            .ok_or_else(missing_field_err("address.first_name"))
    }

    fn get_last_name(&self) -> Result<&Secret<String>, Error> {
        self.last_name
            .as_ref()
            .ok_or_else(missing_field_err("address.last_name"))
    }

    fn get_line1(&self) -> Result<&Secret<String>, Error> {
        self.line1
            .as_ref()
            .ok_or_else(missing_field_err("address.line1"))
    }

    fn get_city(&self) -> Result<&String, Error> {
        self.city
            .as_ref()
            .ok_or_else(missing_field_err("address.city"))
    }

    fn get_line2(&self) -> Result<&Secret<String>, Error> {
        self.line2
            .as_ref()
            .ok_or_else(missing_field_err("address.line2"))
    }

    fn get_zip(&self) -> Result<&Secret<String>, Error> {
        self.zip
            .as_ref()
            .ok_or_else(missing_field_err("address.zip"))
    }

    fn get_country(&self) -> Result<&api_models::enums::CountryAlpha2, Error> {
        self.country
            .as_ref()
            .ok_or_else(missing_field_err("address.country"))
    }

    fn get_combined_address_line(&self) -> Result<Secret<String>, Error> {
        Ok(Secret::new(format!(
            "{},{}",
            self.get_line1()?.peek(),
            self.get_line2()?.peek()
        )))
    }
}

pub trait MandateData {
    fn get_end_date(&self, format: date_time::DateFormat) -> Result<String, Error>;
    fn get_metadata(&self) -> Result<pii::SecretSerdeValue, Error>;
}

impl MandateData for payments::MandateAmountData {
    fn get_end_date(&self, format: date_time::DateFormat) -> Result<String, Error> {
        let date = self.end_date.ok_or_else(missing_field_err(
            "mandate_data.mandate_type.{multi_use|single_use}.end_date",
        ))?;
        date_time::format_date(date, format)
            .into_report()
            .change_context(errors::ConnectorError::DateFormattingFailed)
    }
    fn get_metadata(&self) -> Result<pii::SecretSerdeValue, Error> {
        self.metadata.clone().ok_or_else(missing_field_err(
            "mandate_data.mandate_type.{multi_use|single_use}.metadata",
        ))
    }
}

pub trait MandateReferenceData {
    fn get_connector_mandate_id(&self) -> Result<String, Error>;
}

impl MandateReferenceData for api_models::payments::ConnectorMandateReferenceId {
    fn get_connector_mandate_id(&self) -> Result<String, Error> {
        self.connector_mandate_id
            .clone()
            .ok_or_else(missing_field_err("mandate_id"))
    }
}

pub fn get_header_key_value<'a>(
    key: &str,
    headers: &'a actix_web::http::header::HeaderMap,
) -> CustomResult<&'a str, errors::ConnectorError> {
    get_header_field(headers.get(key))
}

pub fn get_http_header<'a>(
    key: &str,
    headers: &'a http::HeaderMap,
) -> CustomResult<&'a str, errors::ConnectorError> {
    get_header_field(headers.get(key))
}

fn get_header_field(
    field: Option<&http::HeaderValue>,
) -> CustomResult<&str, errors::ConnectorError> {
    field
        .map(|header_value| {
            header_value
                .to_str()
                .into_report()
                .change_context(errors::ConnectorError::WebhookSignatureNotFound)
        })
        .ok_or(report!(
            errors::ConnectorError::WebhookSourceVerificationFailed
        ))?
}

pub fn to_boolean(string: String) -> bool {
    let str = string.as_str();
    match str {
        "true" => true,
        "false" => false,
        "yes" => true,
        "no" => false,
        _ => false,
    }
}

pub fn get_connector_meta(
    connector_meta: Option<serde_json::Value>,
) -> Result<serde_json::Value, Error> {
    connector_meta.ok_or_else(missing_field_err("connector_meta_data"))
}

pub fn to_connector_meta<T>(connector_meta: Option<serde_json::Value>) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let json = connector_meta.ok_or_else(missing_field_err("connector_meta_data"))?;
    json.parse_value(std::any::type_name::<T>()).switch()
}

pub fn to_connector_meta_from_secret<T>(
    connector_meta: Option<Secret<serde_json::Value>>,
) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let connector_meta_secret =
        connector_meta.ok_or_else(missing_field_err("connector_meta_data"))?;
    let json = connector_meta_secret.peek().clone();
    json.parse_value(std::any::type_name::<T>()).switch()
}

impl common_utils::errors::ErrorSwitch<errors::ConnectorError> for errors::ParsingError {
    fn switch(&self) -> errors::ConnectorError {
        errors::ConnectorError::ParsingFailed
    }
}

pub fn base64_decode(data: String) -> Result<Vec<u8>, Error> {
    consts::BASE64_ENGINE
        .decode(data)
        .into_report()
        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
}

pub fn to_currency_base_unit_from_optional_amount(
    amount: Option<i64>,
    currency: storage_models::enums::Currency,
) -> Result<String, error_stack::Report<errors::ConnectorError>> {
    match amount {
        Some(a) => to_currency_base_unit(a, currency),
        _ => Err(errors::ConnectorError::MissingRequiredField {
            field_name: "amount",
        }
        .into()),
    }
}

pub fn to_currency_base_unit(
    amount: i64,
    currency: storage_models::enums::Currency,
) -> Result<String, error_stack::Report<errors::ConnectorError>> {
    utils::to_currency_base_unit(amount, currency)
        .change_context(errors::ConnectorError::RequestEncodingFailed)
}

pub fn to_currency_base_unit_asf64(
    amount: i64,
    currency: storage_models::enums::Currency,
) -> Result<f64, error_stack::Report<errors::ConnectorError>> {
    utils::to_currency_base_unit_asf64(amount, currency)
        .change_context(errors::ConnectorError::RequestEncodingFailed)
}

pub fn str_to_f32<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let float_value = value.parse::<f64>().map_err(|_| {
        serde::ser::Error::custom("Invalid string, cannot be converted to float value")
    })?;
    serializer.serialize_f64(float_value)
}

pub fn collect_values_by_removing_signature(
    value: &serde_json::Value,
    signature: &String,
) -> Vec<String> {
    match value {
        serde_json::Value::Null => vec!["null".to_owned()],
        serde_json::Value::Bool(b) => vec![b.to_string()],
        serde_json::Value::Number(n) => match n.as_f64() {
            Some(f) => vec![format!("{f:.2}")],
            None => vec![n.to_string()],
        },
        serde_json::Value::String(s) => {
            if signature == s {
                vec![]
            } else {
                vec![s.clone()]
            }
        }
        serde_json::Value::Array(arr) => arr
            .iter()
            .flat_map(|v| collect_values_by_removing_signature(v, signature))
            .collect(),
        serde_json::Value::Object(obj) => obj
            .values()
            .flat_map(|v| collect_values_by_removing_signature(v, signature))
            .collect(),
    }
}

pub fn collect_and_sort_values_by_removing_signature(
    value: &serde_json::Value,
    signature: &String,
) -> Vec<String> {
    let mut values = collect_values_by_removing_signature(value, signature);
    values.sort();
    values
}
