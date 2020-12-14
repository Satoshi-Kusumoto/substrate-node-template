#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// https://substrate.dev/docs/en/knowledgebase/runtime/frame

use sp_std::prelude::*;
use frame_system::{
	ensure_signed, ensure_none,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use frame_support::{
	debug, dispatch, decl_module, decl_storage, decl_event, decl_error,
	ensure, storage::IterableStorageMap,
};
use sp_core::crypto::KeyTypeId;
use lite_json::{self, json::JsonValue};

use sp_runtime::{
	transaction_validity::{
		ValidTransaction, InvalidTransaction, TransactionValidity, TransactionSource, TransactionLongevity,
	},
};
use sp_runtime::offchain::http;
use codec::Encode;

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: frame_system::Trait + CreateSignedTransaction<Call<Self>> {
	/// Because this pallet emits events, it depends on the runtime's definition of an event.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type Call: From<Call<Self>>;
}

// The pallet's runtime storage items.
// https://substrate.dev/docs/en/knowledgebase/runtime/storage
decl_storage! {
	// A unique name is used to ensure that the pallet's storage items are isolated.
	// This name may be updated, but each pallet in the runtime must use a unique name.
	// ---------------------------------vvvvvvvvvvvvvv
	trait Store for Module<T: Trait> as TemplateModule {
		// Learn more about declaring storage items:
		// https://substrate.dev/docs/en/knowledgebase/runtime/storage#declaring-storage-items
		EthereumLink get(fn get_address): map hasher(blake2_128_concat) T::AccountId => Option<[u8; 20]>;
		ClaimAccountSet get(fn query_account_set): map hasher(blake2_128_concat) T::AccountId => ();
		AccountBalance get(fn account_balance): map hasher(blake2_128_concat) T::AccountId => u64;
	}
}

// Pallets use events to inform users when important changes are made.
// https://substrate.dev/docs/en/knowledgebase/runtime/events
decl_event!(
	pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		SomethingStored(u32, AccountId),
	}
);

// Errors inform users that something went wrong.
decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Error names should be descriptive.
		InvalidNumber,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}
}

// Dispatchable functions allows users to interact with the pallet and invoke state changes.
// These functions materialize as "extrinsics", which are often compared to transactions.
// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Errors must be initialized if they are used by the pallet.
		type Error = Error<T>;

		// Events must be initialized if they are used by the pallet.
		fn deposit_event() = default;

		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[weight = 10_000]
		pub fn link(origin, addr: [u8; 20]) -> dispatch::DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			let account = ensure_signed(origin)?;
			<EthereumLink<T>>::insert(account, addr);

			Ok(())
		}

		#[weight = 10_000]
		pub fn asset_claim(origin, ) -> dispatch::DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			let account = ensure_signed(origin)?;
			ensure!(!ClaimAccountSet::<T>::contains_key(&account), Error::<T>::InvalidNumber);
			<ClaimAccountSet<T>>::insert(account, ());

			Ok(())
		}

		#[weight = 10_000]
		pub fn clear_claim(origin, block: T::BlockNumber) -> dispatch::DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			ensure_none(origin)?;
			<ClaimAccountSet::<T>>::remove_all();

			Ok(())
		}

		#[weight = 10_000]
		pub fn record_balance(
			origin, 
			account: T::AccountId,
			block: T::BlockNumber,
			balance: u64
		) -> dispatch::DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			ensure_none(origin)?;
			<AccountBalance<T>>::insert(account, balance);

			Ok(())
		}

		fn offchain_worker(block: T::BlockNumber) {
			let accounts: Vec<T::AccountId> = <ClaimAccountSet::<T>>::iter().map(|(k, _)| k).collect();

			if accounts.len() > 0 {
				let call = Call::clear_claim(block);
				let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
				.map_err(|_| {
					debug::error!("Failed to submit unsigned tx");
					<Error<T>>::InvalidNumber;
				});
			}

			match Self::fetch_etherscan(accounts, block) {
				Ok(()) => debug::info!("ocw succeeded!"),
				Err(err) => debug::info!("ocw failed: {:?}", err),
			}
		}

	}
}


impl<T: Trait> Module<T> {

	fn fetch_etherscan(account_vec: Vec<T::AccountId>, block: T::BlockNumber) -> Result<(), Error<T>> {
		for (_, account) in account_vec.iter().enumerate() {
			Self::fetch_etherscan_account(account, block)?;
		}
		Ok(())
	}

	fn fetch_etherscan_account(account: &T::AccountId, block: T::BlockNumber) -> Result<(), Error<T>> {

		// you can use this hardcoded balance, or fetch the hardcoded url, or write you own logic to assemble the url
		let balance: u64 = 100;

		const URL: &str = "https://api-cn.etherscan.com/api?module=account&action=balance&address=0x26d68B9b0c5ad8b2eDdaA71Be3e623c43B79254D&tag=latest";

		let result = Self::fetch_json(URL.as_bytes()).map_err(|_| Error::<T>::InvalidNumber)?;
		let response = sp_std::str::from_utf8(&result).map_err(|_| Error::<T>::InvalidNumber)?;
		let balance = Self::parse_balance(response);

		match balance {
			Some(data) => {
				let balance = Self::chars_to_u64(data).map_err(|_| Error::<T>::InvalidNumber)?;
				let call = Call::record_balance(account.clone(), block, balance);
				let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
				.map_err(|_| {
					debug::error!("Failed to submit unsigned tx");
					<Error<T>>::InvalidNumber;
				});
			}
			_ => (),
		}

		Ok(())
	}

	// Fetch json result from remote URL
	fn fetch_json<'a>(remote_url: &'a [u8]) -> Result<Vec<u8>, &'static str> {
		let remote_url_str = core::str::from_utf8(remote_url)
			.map_err(|_| "Error in converting remote_url to string")?;
	
		debug::info!("Offchain Worker request url is {}.", remote_url_str);
		let pending = http::Request::get(remote_url_str).send()
			.map_err(|_| "Error in sending http GET request")?;
	
		let response = pending.wait()
			.map_err(|e| {
				debug::info!("{:?}", e); "Error in waiting http response back"
			})?;
	
		if response.code != 200 {
			debug::warn!("Unexpected status code: {}", response.code);
			return Err("Non-200 status code returned from http request");
		}

		let json_result: Vec<u8> = response.body().collect::<Vec<u8>>();

		let balance =
			core::str::from_utf8(&json_result).map_err(|_| "JSON result cannot convert to string")?;

		Ok(balance.as_bytes().to_vec())
	}

	// Parse a single balance from ethscan respose
	fn parse_balance(price_str: &str) -> Option<Vec<char>> {
		// {
		// "status": "1",
		// "message": "OK",
		// "result": "3795858430482738500000001"
		// }
		let val = lite_json::parse_json(price_str);
		let balance = val.ok().and_then(|v| match v {
			JsonValue::Object(obj) => {
				obj.into_iter()
					.find(|(k, _)| { 
						let mut chars = "result".chars();
						k.iter().all(|k| Some(*k) == chars.next())
					})
					.and_then(|v| match v.1 {
						JsonValue::String(balance) => Some(balance),
						_ => None,
					})
			},
			_ => None
		})?;
		Some(balance)
	}

	// U64 number string to u64
	pub fn chars_to_u64(vec: Vec<char>) -> Result<u64, &'static str> {
		let mut result: u64 = 0;
		for item in vec {
			let n = item.to_digit(10);
			match n {
				Some(i) => {
					let i_64 = i as u64; 
					result = result * 10 + i_64;
					if result < i_64 {
						return Err("Wrong u64 balance data format");
					}
				},
				None => return Err("Wrong u64 balance data format"),
			}
		}
		return Ok(result)
	}

	// number byte to string byte
	fn u8_to_str_byte(a: u8) -> u8{
		if a < 10 {
			return a + 48 as u8;
		}
		else {
			return a + 87 as u8;
		}
	}

	// address to string bytes
	fn address_to_string(address: &[u8; 20]) -> Vec<u8> {

		let mut vec_result: Vec<u8> = Vec::new();
		for item in address {
			let a: u8 = item & 0x0F;
			let b: u8 = item >> 4;
			vec_result.push(Self::u8_to_str_byte(b));
			vec_result.push(Self::u8_to_str_byte(a));
		}
		return vec_result;
	}

}

impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {

		match call {
		Call::record_balance(account, block, price) => Ok(ValidTransaction {
			priority: 0,
			requires: vec![],
			provides: vec![(account, block, price).encode()],
			longevity: TransactionLongevity::max_value(),
			propagate: true,
		}),
		
		Call::clear_claim(block) => Ok(ValidTransaction {
			priority: 0,
			requires: vec![],
			provides: vec![(block).encode()],
			longevity: TransactionLongevity::max_value(),
			propagate: true,
		}),
		_ => InvalidTransaction::Call.into()
		}
	}
}
