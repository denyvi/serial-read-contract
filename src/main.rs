pub(crate) mod logger;

use futures::stream::StreamExt;
use std::{env, io, str};
use tokio_util::codec::{Decoder, Encoder};

use bytes::BytesMut;
use tokio_serial::SerialPortBuilderExt;

use contract_transcode::ContractMessageTranscoder;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use pallet_contracts_primitives::ContractExecResult;
use parity_scale_codec::{Decode, Encode};
use sp_core::crypto::{set_default_ss58_version, AccountId32, Ss58Codec};
use sp_core::Bytes;
use ss58_registry::Ss58AddressFormatRegistry;
use std::time::Duration;

pub const CONTRACT_CALLER: &str = "5ExcvnRUfE9dWBgma5DCVeENgiq2jEo1cY4pW7J8yqvjTE3C";
pub const CONTRACT_ADDRESS: &str = "gr4LugUgbox1qh3JdjMsecmqREXvDCmAwVc5GhyhQjgei3HyS";
pub const CONTRACT_METADATA: &str = "./resources/rantai_suplai.json";
pub const CONTRACT_METHOD: &str = "get_product_status";

#[derive(Encode)]
pub struct Weight {
    proof_size: u64,
    ref_time: u64,
}
#[derive(Encode)]
pub struct ContractCallEnvelope {
    origin: AccountId32,
    dest: AccountId32,
    value: u128,
    gas_limit: Option<Weight>,
    storage_deposit_limit: Option<u128>,
    input_data: Vec<u8>,
}

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

struct LineCodec;

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let newline = src.as_ref().iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let line = src.split_to(n + 1);
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Invalid String")),
            };
        }
        Ok(None)
    }
}

impl Encoder<String> for LineCodec {
    type Error = io::Error;

    fn encode(&mut self, _item: String, _dst: &mut BytesMut) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init_logger();
    
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());
    
    let mut port = tokio_serial::new(tty_path, 115200).open_native_async()?;

    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    let mut reader = LineCodec.framed(port);

    while let Some(line_result) = reader.next().await {
        let line = line_result.expect("Failed to read line");
        let parsed_line: Vec<&str> = line.split("-").collect();
        let batch_id = parsed_line[0].trim();
        let product_id = parsed_line[1].trim();
        let quote_char = "\"";
        let mod_batch_id = format!("{}{}{}", quote_char,batch_id,quote_char);
        let mod_product_id = format!("{}{}{}", quote_char,product_id,quote_char);
        let msg_args = [mod_batch_id, mod_product_id];
        // let msg_args = [stringify!("AAA"), stringify!("2")];
        
        set_default_ss58_version(Ss58AddressFormatRegistry::GoroAccount.into());
        let contract_transcoder = ContractMessageTranscoder::load(CONTRACT_METADATA)?;
        let contract_caller = AccountId32::from_ss58check(CONTRACT_CALLER)?;
        let contract_address = AccountId32::from_ss58check(CONTRACT_ADDRESS)?;

        let contract_message_data =
            contract_transcoder.encode(CONTRACT_METHOD, msg_args)?;

        let contract_call_envelope = ContractCallEnvelope {
            origin: contract_caller,
            dest: contract_address,
            value: 0,
            gas_limit: None,
            storage_deposit_limit: None,
            input_data: contract_message_data,
        };

        let contract_call_envelope_bytes = contract_call_envelope.encode();
        let contract_call_envelope_as_rpc_params = rpc_params!["ContractsApi_call", Bytes(contract_call_envelope_bytes)];
        let goro_client = WsClientBuilder::default()
            .max_concurrent_requests(1)
            .ping_interval(Duration::from_secs(1))
            .request_timeout(Duration::from_secs(5))
            .build("wss://main-00.goro.network:443")
            .await?;

        let result: Bytes = goro_client
            .request("state_call", contract_call_envelope_as_rpc_params)
            .await?;
        let contract_exec_result = ContractExecResult::<u128>::decode(&mut result.as_ref())?;

        match contract_exec_result.result {
            Err(err) => {
                logger::error!("{err:?}");
            }
            Ok(ref exec_return_value) => {
                let value =
                    contract_transcoder.decode_return(CONTRACT_METHOD, &mut &exec_return_value.data[..])?;
                logger::info!("Result => {}", value);
            }
        }
    }
    Ok(())
}
