 use glitch_runtime::{
    AccountId, Balance, BalancesConfig,
    EVMConfig, EthereumConfig, GenesisConfig, ImOnlineId, IndicesConfig,
    SessionConfig, SessionKeys, Signature, StakerStatus, StakingConfig, SudoConfig, SystemConfig,
    DOLLARS,CENTS,MILLICENTS, WASM_BINARY
};
use cumulus_primitives_core::ParaId;
use pallet_evm::GenesisAccount;
use primitive_types::H160;
use sc_service::ChainType;
use serde_json as json;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_core::{sr25519, Pair, Public, U256};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    Perbill,
};
use std::collections::BTreeMap;
use std::str::FromStr;
use sc_telemetry::TelemetryEndpoints;
use hex_literal::hex;
use sp_core::crypto::UncheckedInto;
use log::warn;
use serde::{Deserialize, Serialize};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use pallet_staking::Forcing;

// The URL for the telemetry server.
// const TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

const DEFAULT_PROPERTIES_TESTNET: &str = r#"
{
"tokenSymbol": "GLCH",
"tokenDecimals": 18,
"ss58Format": 42
}
"#;

const DEFAULT_PROPERTIES_MAINNET: &str = r#"
{
    "tokenSymbol": "GLCH",
    "tokenDecimals": 18,
    "ss58Format": 42
}
"#;

fn session_keys(
    grandpa: GrandpaId,
    babe: BabeId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
    SessionKeys {
        grandpa,
        babe,
        im_online,
        authority_discovery,
    }
}

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
    /// The relay chain of the Parachain.
    pub relay_chain: String,
    /// The id of the Parachain.
    pub para_id: u32,
    /// The light sync state extension used by the sync-state rpc.
    pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

impl Extensions {
    /// Try to get the extension from the given `ChainSpec`.
    pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
        sc_chain_spec::get_extension(chain_spec.extensions())
    }
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate authority key.
pub fn authority_keys_from_seed(s: &str) -> (
    AccountId,
    GrandpaId,
    BabeId,
    ImOnlineId,
    AuthorityDiscoveryId,
) {
  (
    get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", s)),
    get_from_seed::<GrandpaId>(s),
    get_from_seed::<BabeId>(s),
    get_from_seed::<ImOnlineId>(s),
    get_from_seed::<AuthorityDiscoveryId>(s),
  )
}

fn endowed_evm_account() -> BTreeMap<H160, GenesisAccount> {
    let endowed_account = vec![];
    get_endowed_evm_accounts(endowed_account)
}

fn dev_endowed_evm_accounts() -> BTreeMap<H160, GenesisAccount> {
    let endowed_account = vec![
        H160::from_str("8097c3C354652CB1EEed3E5B65fBa2576470678A").unwrap(),
        H160::from_str("6be02d1d3665660d22ff9624b7be0551ee1ac91b").unwrap(),
        H160::from_str("e6206C7f064c7d77C6d8e3eD8601c9AA435419cE").unwrap(),
        // the dev account key
        // seed: bottom drive obey lake curtain smoke basket hold race lonely fit walk
        // private key: 0x03183f27e9d78698a05c24eb6732630eb17725fcf2b53ee3a6a635d6ff139680
        H160::from_str("aed40f2261ba43b4dffe484265ce82d8ffe2b4db").unwrap(),
    ];

    get_endowed_evm_accounts(endowed_account)
}

fn get_endowed_evm_accounts(endowed_account: Vec<H160>) -> BTreeMap<H160, GenesisAccount> {
    let mut evm_accounts = BTreeMap::new();
    for account in endowed_account {
        evm_accounts.insert(
            account,
            GenesisAccount {
                nonce: U256::from(0),
                balance: U256::from(0 * DOLLARS),
                storage: Default::default(),
                code: vec![],
            },
        );
    }
    evm_accounts
}

pub fn development_config(id: ParaId) -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Development",
        // ID
        "dev",
        ChainType::Development,
        move || {
            glitch_genesis(
                wasm_binary,
                // Initial PoA authorities
                vec![authority_keys_from_seed("Alice")],
                // Sudo account
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                // Pre-funded accounts
                vec![],
                true,
                dev_endowed_evm_accounts(),
                id,
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        Some("glitch_nodelocal"),
        None,
        // Properties
        Some(json::from_str(DEFAULT_PROPERTIES_TESTNET).unwrap()),
        // Extensions
        Default::default(),
    ))
}

pub fn local_testnet_config(id: ParaId) -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "glitch_node",
        // ID
        "local_testnet",
        ChainType::Local,
        move || {
            glitch_genesis(
                wasm_binary,
                // Initial PoA authorities
                vec![
                    authority_keys_from_seed("Alice"),
                    authority_keys_from_seed("Bob"),
                ],
                // Sudo account
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                // Pre-funded accounts
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie"),
                    get_account_id_from_seed::<sr25519::Public>("Dave"),
                    get_account_id_from_seed::<sr25519::Public>("Eve"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                    //get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    //get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
                ],
                true,
                endowed_evm_account(),
                id,
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        Some("glitch_nodelocal"),
        None,
        // Properties
        Some(json::from_str(DEFAULT_PROPERTIES_TESTNET).unwrap()),
        // Extensions
        Default::default(),
    ))
}

//Glitch testnet
pub fn glitch_testnet_config(id: ParaId) -> Result<ChainSpec, String> {
  let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
  Ok(ChainSpec::from_genesis(
      //Name
      "Glitch",
      //ID
      "glitch_testnet",
      ChainType::Local,
      move || glitch_genesis(
          wasm_binary,
          // Initial PoA authories
          vec![
            // SECRET=""
            // subkey inspect "$SECRET//glitch//1//validator"
            // subkey inspect "$SECRET//glitch//1//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//1//grandpa"
            // subkey inspect "$SECRET//glitch//1//imonline"
            // subkey inspect "$SECRET//glitch//1//discovery"
            (
              hex!["aee0df231aed47a99beb26960d156de55751a48741b1cfcfa99e42a5f499e63f"].into(),
              hex!["04315411f66a58e838690017ac4102374e4f1dd3f429176ebaa15d55d61d9e80"].unchecked_into(), // grandpa
              hex!["2eed8720f0cc2697ef251da006980c0873cfc108f4c91a96744eedf1b59aff44"].unchecked_into(), // babe key
              hex!["a86ab6e2458078bc49f7277ba00642c1ba2336f8001dd7de6494fd0751a1c057"].unchecked_into(), // imonline
              hex!["1eaa69cbee5e16337c52440455fbc97b839d01bdc4e6d158cf3061d44ba4fc7b"].unchecked_into(), // discovery
            ),
            // SECRET=""
            // 5G6nMq5x8xm3PLxyKXkKEkzAjzQLGbuiDarjBWp6d5XHg3FM
            // subkey inspect "$SECRET//glitch//2//validator"
            // subkey inspect "$SECRET//glitch//2//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//2//grandpa"
            // subkey inspect "$SECRET//glitch//2//imonline"
            // subkey inspect "$SECRET//glitch//2//discovery"
            (
              hex!["5a44973835f643d6d2c45c4490e1c0095755d469ae5db20d480c937588abd164"].into(),
              hex!["72f22b083c995d0c4bf07a46f7ad326bcc25780483c8eb0523f53ed2a5b7915d"].unchecked_into(), // grandpa
              hex!["381bab65c6c3c0a9bfb06e7a6c8a3b109bed9e97303b22fcd0157ba0871a6566"].unchecked_into(), // babe
              hex!["46bd8c5f164df0a6db9199a74376aea3cf8f0d4ecc8b4da5289ebf70c6d0496a"].unchecked_into(), // imonline
              hex!["98a818de9aa0ea6d376a8690bfebb24b1d5d7d9c6f36e1ab45983b0fa7f2e328"].unchecked_into(), // discovery
            ),
            // SECRET=""
            // subkey inspect "$SECRET//glitch//3//validator"
            // subkey inspect "$SECRET//glitch//3//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//3//grandpa"
            // subkey inspect "$SECRET//glitch//3//imonline"
            // subkey inspect "$SECRET//glitch//3//discovery"
            (
              hex!["94f1d376734418cc137900c7ad4bb5a4911af24d00e4fd72803e3817a50c334d"].into(),
              hex!["0b9f59981f7b9a654f9819c7cb774f3fe39fc9e4c5ac22202f77be52677a5fd3"].unchecked_into(), // grandpa
              hex!["6eb9b6f2680b69714d38d0d34108288c9498616589afa6ca453f7a71a88cc64c"].unchecked_into(), // babe
              hex!["1efe62dcf7953227eb99413539b2d58c584f8568ab72d847310a17030882ef0d"].unchecked_into(), // imonline
              hex!["82aba2132ab4ff2e7fcafeb4d4932f73413d08b7f3a54e18ea2c8d4cffa88e2b"].unchecked_into(), // discovery
            ),
          ],
          // 5D5MHV7hy6LTcRUSL3cVYhGecar59RAj1UwnLLEFikUySxsX
          hex!["2cba3a171f0eac2c17fb96e1172de8bdfb5977d687be988c8c9c424e89ccf017"].into(),
          vec![
            // 5D5MHV7hy6LTcRUSL3cVYhGecar59RAj1UwnLLEFikUySxsX
            hex!["2cba3a171f0eac2c17fb96e1172de8bdfb5977d687be988c8c9c424e89ccf017"].into(),
            //5H6ji2UhbPKFmhHa66ETXHmpwGTNLtZEnoJBPcT1eBL2nf9A
            hex!["deb9e399fc7a8dbfdb3afdded6d944001f06d34f2ee44f221214e5f651d72d0c"].into(),
            //5F7hrtWMqodEtbEMCvdTWwRMW2iCPsp7vjmMkUqCMAZjew6k
            hex!["86fe730862e66d0843fd26317cdc8982e964411c6b567ede64f5b75256e3ae02"].into(),
            //5E77ca5cwnKBK4qhc2nQQSsSz5oUbvop84Y2yVW1TPDR7AZg
            hex!["5a4eee4225fdfeea0affc8b3898b6473aa5da97acb5cb77eff87e85da9cd442a"].into(),
            //5DRnNDpt5uVaVD8RsjAZCpZUWqx6nCZFRNaCKNR1qFEqggof
            hex!["3c4f9141446d18bc60cde8b084fff57830de18e05657a113b7eb55a89cf1ba77"].into(),
            //5EFiphQLB8sRtbYdE6vhf6Gs5yGeTUcPkkxsu7cehhn2mLBp
            hex!["60df6b11a2d8d6460fd583e491a52f0518b515d29b63ae0fca6ad04dd99c9820"].into(),
            //5EnkYVT7Dgr8gYzFPE4vJLdy9YCJt9qnccpYEREbedo9KSRw
            hex!["7889d159ed22e156047c91c03c49d0fdb97db5aa121dd248fc9143ae58605a78"].into(),
            //5GerXFwjqzasX2R8ND92dbCq7dscharx2TcMCWCFZQNkva46
            hex!["cafc729c1e7c5d13065c41e39dba17f81f2c37ffa59d55a058bcc1eea3a7bd0d"].into(),
            //5FxvMhxNswBFYyUw3Tu7d1NSnGnFHPEpSwsL9kKp6ZJyGGbP
            hex!["ac878ad7a137e325872d74022e85fe976d26c5b51a172821b7a8ca6526a34928"].into(),
            //5HWd37PNa5Vehsy4196fgA1uEG1o3CPySWC2urt35CMi9ipR
            hex!["f0f15284b61f6ed6a3fc292395e0fa3195e79052b1ec8bca74298f5de5134754"].into(),
          ],
          true,
          endowed_evm_account(),
          id,
      ),
      // Bootnodes
      // node-key=0decb1a3d303a8849a06e9c258698929ee1dfdc524fddc7be1771becd7236e29
      vec![
           "/dns/glitch-bootnode.sotatek.works/tcp/30333/p2p/12D3KooWFKSEVZGNrS6THQ6J2vSgLDePdXXz9HYE6TtgopZV22T1"
        .parse()
        .unwrap(),
          "/ip4/10.2.15.53/tcp/30333/p2p/12D3KooWFKSEVZGNrS6THQ6J2vSgLDePdXXz9HYE6TtgopZV22T1"
        .parse()
        .unwrap(),
      ],
      //Telemetry
      None,
      // Protocol ID
      Some("glitch_testnet"),
      None,
      // Properties
      Some(json::from_str(DEFAULT_PROPERTIES_TESTNET).unwrap()),
      // Extension
      Default::default(),
  ))
}

//Glitch Mainnet
pub fn glitch_mainnet_config(id: ParaId) -> Result<ChainSpec, String> {
  let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
  Ok(ChainSpec::from_genesis(
      //Name
      "Glitch",
      //ID
      "glitch_mainnet",
      ChainType::Live,
      move || glitch_genesis(
          wasm_binary,
          // Initial PoA authories
          vec![
        // SECRET=""
        // subkey inspect "$SECRET//glitch//1//validator"
        // subkey inspect "$SECRET//glitch//1//babe"
        // subkey inspect --scheme ed25519 "$SECRET//glitch//1//grandpa"
        // subkey inspect "$SECRET//glitch//1//imonline"
        // subkey inspect "$SECRET//glitch//1//discovery"
        (
          hex!["68f1d67412d6528992d391025dbc36a3261c1efaf22066dff692b2e4c1fb726a"].into(),
          hex!["04315411f66a58e838690017ac4102374e4f1dd3f429176ebaa15d55d61d9e80"].unchecked_into(), // grandpa
          hex!["2eed8720f0cc2697ef251da006980c0873cfc108f4c91a96744eedf1b59aff44"].unchecked_into(), // babe key
          hex!["a86ab6e2458078bc49f7277ba00642c1ba2336f8001dd7de6494fd0751a1c057"].unchecked_into(), // imonline
          hex!["1eaa69cbee5e16337c52440455fbc97b839d01bdc4e6d158cf3061d44ba4fc7b"].unchecked_into(), // discovery
        ),
        // SECRET=""
        // 5G6nMq5x8xm3PLxyKXkKEkzAjzQLGbuiDarjBWp6d5XHg3FM
        // subkey inspect "$SECRET//glitch//2//validator"
        // subkey inspect "$SECRET//glitch//2//babe"
        // subkey inspect --scheme ed25519 "$SECRET//glitch//2//grandpa"
        // subkey inspect "$SECRET//glitch//2//imonline"
        // subkey inspect "$SECRET//glitch//2//discovery"
        (
          hex!["2600c75f5fe2ddb65676361769e637069cb2041622979ba118a68993279deb0b"].into(),
          hex!["72f22b083c995d0c4bf07a46f7ad326bcc25780483c8eb0523f53ed2a5b7915d"].unchecked_into(), // grandpa
          hex!["381bab65c6c3c0a9bfb06e7a6c8a3b109bed9e97303b22fcd0157ba0871a6566"].unchecked_into(), // babe
          hex!["46bd8c5f164df0a6db9199a74376aea3cf8f0d4ecc8b4da5289ebf70c6d0496a"].unchecked_into(), // imonline
          hex!["98a818de9aa0ea6d376a8690bfebb24b1d5d7d9c6f36e1ab45983b0fa7f2e328"].unchecked_into(), // discovery
        ),
        // SECRET=""
        // subkey inspect "$SECRET//glitch//3//validator"
        // subkey inspect "$SECRET//glitch//3//babe"
        // subkey inspect --scheme ed25519 "$SECRET//glitch//3//grandpa"
        // subkey inspect "$SECRET//glitch//3//imonline"
        // subkey inspect "$SECRET//glitch//3//discovery"
        (
          hex!["5c790cbdc11a4bf8934250eb27bf26f2ea05db67b4f5fa48a760bcfd9ef43b49"].into(),
          hex!["0b9f59981f7b9a654f9819c7cb774f3fe39fc9e4c5ac22202f77be52677a5fd3"].unchecked_into(), // grandpa
          hex!["6eb9b6f2680b69714d38d0d34108288c9498616589afa6ca453f7a71a88cc64c"].unchecked_into(), // babe
          hex!["1efe62dcf7953227eb99413539b2d58c584f8568ab72d847310a17030882ef0d"].unchecked_into(), // imonline
          hex!["82aba2132ab4ff2e7fcafeb4d4932f73413d08b7f3a54e18ea2c8d4cffa88e2b"].unchecked_into(), // discovery
        ),
      ],
          // root
          hex!["d8d222c44e7b678dc07a1136ca146c8cf71d46d7327b67ab3d6abe3b4f83cb3d"].into(),
          vec![
              hex!["1608a5e4d16f4b694a164372e1fd5af8944514d7cec9263fec457bac96e25565"].into(),
              hex!["9c5f9d91b99f8b1f25cf075ba57839734f3e249b72adcf04899a46c8cfd95b4e"].into(),
              hex!["501934c8d7b257fbadd003bb4a29a5adb9fce7fa8d659e28d473002f5fffcf65"].into(),
              hex!["c4a01a3a57602229e112de1449d83cfbbfbc6d423da7e0ab7baeacfcf83f1d2f"].into(),
              hex!["d8412bac516c4c079016a8ae6eefb983837274d884868dc9528e6a2a27dfbb0d"].into(),
              hex!["1c282c8b1e00a3b7ba2d7a4466a23420cb398aa6e53ca8a3ec75f92cfd93e97b"].into(),
              hex!["1290c0fa454d01631a80f6b62dd080ed4e1a76c9cdef6045594dc7e2d226eb44"].into(),
              hex!["4e5b90d22cb365beeed1a96ffea8175e30daacb628898998810f57ec65cf7969"].into(),
              hex!["66b4c4464cfd187ae8206a29fe7079ec156d37af10d763e5171da19d66bca742"].into(),
              hex!["004028fd0cf9675e2c1698c5c539f5b273e73c490cd3fb54f32923e630e66922"].into(),
          ],
          true,
          endowed_evm_account(),
          id,
      ),
      // Bootnodes
      // node-key=0decb1a3d303a8849a06e9c258698929ee1dfdc524fddc7be1771becd7236e29
      vec![
           "/dns/fullnodes-mainnet-1.glitch.finance/tcp/30333/p2p/12D3KooWFKSEVZGNrS6THQ6J2vSgLDePdXXz9HYE6TtgopZV22T1"
        .parse()
        .unwrap(),
          "/dns/validatornodes-mainnet-1.glitch.finance/tcp/30333/p2p/12D3KooWPRSGH3LnwG5Uhj9Nm7qY7hkdrhd4vg4znb49ix9ADZwD"
        .parse()
        .unwrap(),
          "/dns/validatornodes-mainnet-2.glitch.finance/tcp/30333/p2p/12D3KooWBEyY6ySQqjniaqVHH1JtiMVU5KmSPvoPgqkpp6XhBjEt"
        .parse()
        .unwrap(),
          "/dns/validatornodes-mainnet-3.glitch.finance/tcp/30333/p2p/12D3KooWKwDVXckeQ86PrNFCyrbaA4sEkg9hPY9a5fe2yZ3gSRX1"
        .parse()
        .unwrap(),
          "/dns/validatornodes-mainnet-4.glitch.finance/tcp/30333/p2p/12D3KooWRYMq2fD7cRikXXc9doocmBNw37tfQTKSnPfEfqeaMyG3"
        .parse()
        .unwrap(),
      ],
      //Telemetry
      None,
      // Protocol ID
      Some("glitch_mainnet"),
      None,
      // Properties
      Some(json::from_str(DEFAULT_PROPERTIES_MAINNET).unwrap()),
      // Extension
      Default::default(),
  ))
}

// Glitch UAT
pub fn glitch_uat_config(id: ParaId) -> Result<ChainSpec, String> {
  let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
  Ok(ChainSpec::from_genesis(
      //Name
      "Glitch UAT",
      //ID
      "glitch_uat",
      ChainType::Live,
      move || glitch_genesis(
          wasm_binary,
          // Initial PoA authories
          vec![
            // SECRET=""
            // subkey inspect "$SECRET//glitch//1//validator"
            // subkey inspect "$SECRET//glitch//1//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//1//grandpa"
            // subkey inspect "$SECRET//glitch//1//imonline"
            // subkey inspect "$SECRET//glitch//1//discovery"
            (
              hex!["a25483fe9cca2461c83a00513f07377f34116fc35f5fefec53fca9e59d1d2f06"].into(),
              hex!["04315411f66a58e838690017ac4102374e4f1dd3f429176ebaa15d55d61d9e80"].unchecked_into(), // grandpa
              hex!["2eed8720f0cc2697ef251da006980c0873cfc108f4c91a96744eedf1b59aff44"].unchecked_into(), // babe key
              hex!["a86ab6e2458078bc49f7277ba00642c1ba2336f8001dd7de6494fd0751a1c057"].unchecked_into(), // imonline
              hex!["1eaa69cbee5e16337c52440455fbc97b839d01bdc4e6d158cf3061d44ba4fc7b"].unchecked_into(), // discovery
            ),
            // SECRET=""
            // 5G6nMq5x8xm3PLxyKXkKEkzAjzQLGbuiDarjBWp6d5XHg3FM
            // subkey inspect "$SECRET//glitch//2//validator"
            // subkey inspect "$SECRET//glitch//2//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//2//grandpa"
            // subkey inspect "$SECRET//glitch//2//imonline"
            // subkey inspect "$SECRET//glitch//2//discovery"
            (
              hex!["1408fed1c2d32cf914326b1ec33edbeb4427fdaf411e27f9dd77b6bcaf280f54"].into(),
              hex!["72f22b083c995d0c4bf07a46f7ad326bcc25780483c8eb0523f53ed2a5b7915d"].unchecked_into(), // grandpa
              hex!["381bab65c6c3c0a9bfb06e7a6c8a3b109bed9e97303b22fcd0157ba0871a6566"].unchecked_into(), // babe
              hex!["46bd8c5f164df0a6db9199a74376aea3cf8f0d4ecc8b4da5289ebf70c6d0496a"].unchecked_into(), // imonline
              hex!["98a818de9aa0ea6d376a8690bfebb24b1d5d7d9c6f36e1ab45983b0fa7f2e328"].unchecked_into(), // discovery
            ),
            // SECRET=""
            // subkey inspect "$SECRET//glitch//3//validator"
            // subkey inspect "$SECRET//glitch//3//babe"
            // subkey inspect --scheme ed25519 "$SECRET//glitch//3//grandpa"
            // subkey inspect "$SECRET//glitch//3//imonline"
            // subkey inspect "$SECRET//glitch//3//discovery"
            (
              hex!["3443862d2ff8f750e46ff0424ad541b8a34896a3112babf96054362325c49977"].into(),
              hex!["0b9f59981f7b9a654f9819c7cb774f3fe39fc9e4c5ac22202f77be52677a5fd3"].unchecked_into(), // grandpa
              hex!["6eb9b6f2680b69714d38d0d34108288c9498616589afa6ca453f7a71a88cc64c"].unchecked_into(), // babe
              hex!["1efe62dcf7953227eb99413539b2d58c584f8568ab72d847310a17030882ef0d"].unchecked_into(), // imonline
              hex!["82aba2132ab4ff2e7fcafeb4d4932f73413d08b7f3a54e18ea2c8d4cffa88e2b"].unchecked_into(), // discovery
            ),
          ],
          //root:
          hex!["88b4fc7317577d1582969bbc2c3e179926e07c88a7507302fec5fd4f662a9567"].into(),
          vec![
              hex!["88b4fc7317577d1582969bbc2c3e179926e07c88a7507302fec5fd4f662a9567"].into(),
          ],
          true,
          endowed_evm_account(),
          id,
      ),
      // Bootnodes
      // node-key=0decb1a3d303a8849a06e9c258698929ee1dfdc524fddc7be1771becd7236e29
      vec![
           "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWE9d5iYYuom8HZnPVcBcWZ9vdJQrM8pSjJJuGBJywcWDx"
        .parse()
        .unwrap() , 
      ],
      //Telemetry
      None,
      // Protocol ID
      Some("glitch_uat"),
      None,
      // Properties
      Some(json::from_str(DEFAULT_PROPERTIES_MAINNET).unwrap()),
      // Extension
      Default::default(),
  ))
}

/// Configure initial storage state for FRAME modules.
fn glitch_genesis(
    wasm_binary: &[u8],
    initial_authorities: Vec<(
        AccountId,
        GrandpaId,
        BabeId,
        ImOnlineId,
        AuthorityDiscoveryId,
    )>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
    endowed_eth_accounts: BTreeMap<H160, GenesisAccount>,
    id: ParaId,
) -> GenesisConfig {
    let enable_println = true;
    const TOTAL_SUPPLY: Balance = 88_888_888 * DOLLARS;
    const STASH: Balance = 88_888 * DOLLARS;
    const AUTHOR_BALANCE: Balance = 330_000 * DOLLARS;

    let validator_count = Balance::from(initial_authorities.len() as u32);
    let total_endowment: Balance = TOTAL_SUPPLY - AUTHOR_BALANCE * validator_count - 3 * DOLLARS / 10;
    let endowed_count = Balance::from(endowed_accounts.len() as u32);

    let endowment: Balance =
        if endowed_count != 0{
            if total_endowment % endowed_count != 0{
                panic!();
            }
            total_endowment / endowed_count
        }else{
            0
        };
    
    warn!(
        "--------------------------------------------------------\n \
        endowment: {:?}, STASH: {:?}, AUTHOR_BALANCE: {:?} .     \n \
        ---------------------------------------------------------\n",
        endowment, STASH, AUTHOR_BALANCE
    );


    GenesisConfig {
        system: SystemConfig {
            // Add Wasm runtime to storage.
            code: wasm_binary.to_vec(),
        },
        balances: BalancesConfig {
            // Configure endowed accounts with initial balance of 1 << 60.
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, endowment))
                .chain(
                    initial_authorities
                        .iter()
                        .map(|x| (x.0.clone(), AUTHOR_BALANCE)),
                )
                .collect(),
        },
        /*contracts: ContractsConfig {
          current_schedule: pallet_contracts::Schedule::default()
          .enable_println(enable_println),
        },*/
        evm: EVMConfig {
          accounts: endowed_eth_accounts,
        },
        ethereum: EthereumConfig {},
        indices: IndicesConfig { indices: vec![] },
        parachain_info: glitch_runtime::ParachainInfoConfig { parachain_id: id },
        collator_selection: glitch_runtime::CollatorSelectionConfig {
          invulnerables: initial_authorities
            .iter()
            .cloned()
            .map(|x| x.0.clone())
            .collect(),
          candidacy_bond: 1 * DOLLARS,
          ..Default::default()
        },
        council: Default::default(),
        technical_committee: Default::default(),
        democracy: Default::default(),
        treasury: Default::default(),
        elections_phragmen: Default::default(),
        technical_membership: Default::default(),
        vesting: Default::default(),
        session: glitch_runtime::SessionConfig {
          keys: initial_authorities
            .iter()
            .cloned()
            .map(|(acc, grandpa, babe, im_online, authority_discovery)| {
              (
                acc.clone(),        // account id
                acc.clone(),        // validator id
                session_keys(grandpa, babe, im_online, authority_discovery), // session keys
              )
            })
            .collect(),
        },
        //base_fee: Default::default(),
        sudo: SudoConfig {
            // Assign network admin rights.
            key: Some(root_key),
        },
        staking: glitch_runtime::StakingConfig {
            validator_count: initial_authorities.len() as u32,
            minimum_validator_count: initial_authorities.len() as u32,
            stakers: initial_authorities
                .iter()
                .map(|x| (x.0.clone(), x.0.clone(), STASH, StakerStatus::Validator))
                .collect(),
            invulnerables: vec![],
            force_era: Forcing::NotForcing,
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        },
        babe: glitch_runtime::BabeConfig {
          authorities: vec![],
          epoch_config: Some(glitch_runtime::BABE_GENESIS_EPOCH_CONFIG),
        },
        grandpa: glitch_runtime::GrandpaConfig{
          authorities: vec![],
        },
        im_online: Default::default(),
        authority_discovery: glitch_runtime::AuthorityDiscoveryConfig { keys: vec![] },
    }
}
