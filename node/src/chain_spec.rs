 use glitch_runtime::{
    AccountId, Balance, BalancesConfig,
    EVMConfig, EthereumConfig, GenesisConfig, ImOnlineId, IndicesConfig,
    SessionKeys, Signature, StakerStatus, SudoConfig, SystemConfig,
    RevenueConfig,
    DOLLARS, WASM_BINARY
};
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
use hex_literal::hex;
use sp_core::crypto::UncheckedInto;
use log::warn;
use serde::{Deserialize, Serialize};
use sc_chain_spec::{ChainSpecExtension};
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

pub fn development_config() -> Result<ChainSpec, String> {
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

pub fn local_testnet_config() -> Result<ChainSpec, String> {
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
pub fn glitch_testnet_config() -> Result<ChainSpec, String> {
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
            (
                hex!["2ce3d89f76f777476746406f62f79658dc17bc5cbe8cd85cf7491ca353742c0e"].into(),
                hex!["085962c644f36cbe4ce4d92f3c848cf21a1369b3678e79c078eecec1cdf01d02"].unchecked_into(), //grandpa
                hex!["6e6c39de041b5a0580d3e5a86d1453d6235fab204e13042ec4479d363d1ecb6f"].unchecked_into(), //babe
                hex!["ca68b1fa7c7f3ac51615c1f9b483b796afe9855a8ceb7358b8485d95abce7654"].unchecked_into(), //imonline
                hex!["30f5d3317fbf7da7e69410c5383fdd828fc0611522649c5d21955a376de3ad2a"].unchecked_into(), //discovery
            ),
            (
                hex!["cc3da9dca94c890de1d050c832855ccdda94579a0cd36d0e245a1dbec0806271"].into(),
                hex!["ff202396ee774f63dc97038865769fd18d1eda980951f87ac7bdacc812e7b0cf"].unchecked_into(), //grandpa
                hex!["7ee1ade6cf9fade60e491eeb6577a4d6fd567f64155c338986831477935ea443"].unchecked_into(), //babe
                hex!["a46b0a9dfe4e5676b259b3ff2136dd8a5c9e9d12cdacaf6325a713c3e3408272"].unchecked_into(), //imonline
                hex!["7209952c0504e1f9d51bfbd6a956d3c058b516114bf6a9ca5a25c0415542b508"].unchecked_into(), //discovery
            ),
            (
                hex!["066721a1c0cc5fdce2986811ebcc1aef53d9c6a995d67f67a17a2895ab4e787f"].into(),
                hex!["f16a14da8ca4453f60f9ff4be791b0d622df3a42f7c651c84faeb4e44a718a8f"].unchecked_into(), //grandpa
                hex!["d8cf2e5805ee93bc6f607587f900e638c49df4a1f64eee1371a2daee71f83629"].unchecked_into(), //babe
                hex!["2e9d53acda0bdcb0e00f7e92735e5b22d225aff6104f6f3508000c227fdde003"].unchecked_into(), //imonline
                hex!["a688194af51863a9e919baf53552b87c1c4012a8cae5ff6f877d08c420dd2f19"].unchecked_into(), //discovery
            ),
            (
                hex!["ae888b76303a620b67da569314967a4508aef18298ae5706c47440086f1fa70b"].into(),
                hex!["28e9cd3c0cb9267cc3fdf0c9a987b32817dc4ea8d6d6ca16f940e268236d806f"].unchecked_into(), //grandpa
                hex!["a4154f924c97a0164f853377752687ec12ca9ff124ec2c8321d6ab5464d4e144"].unchecked_into(), //babe
                hex!["6ec2030938cfc02fccb91813b1e46c0bd2578e09972aaaab0baf7e9cf913e230"].unchecked_into(), //imonline
                hex!["9e91bcc2325f5cff26e3336a8541b463e9ca7b68d03841371c1c71cf2cda440f"].unchecked_into(), //discovery
            ),
            (
                hex!["a2916e1509eac2f876ec0206dfbb21ed899dc4241182d33b6a208e993e42bb0a"].into(),
                hex!["f2e06e37a6a102d05160ff305080454330286136a9ad39a96a4aa7d5acc9be45"].unchecked_into(), //grandpa
                hex!["646436409fcde2e9fdf5fb25439512e4d66c1b348263c3d71959d69311dbe43c"].unchecked_into(), //babe
                hex!["c6c9eca067f3dc54a3ea566be9ab73cf9714d887353fdbc5e3ae0921ea0be074"].unchecked_into(), //imonline
                hex!["b8b5fe534bdea88a251c5d2caf7aeec0e2201d0a0cebe23ee7a215298fa9a77d"].unchecked_into(), //discovery
            ),
          ],
          // 5D5MHV7hy6LTcRUSL3cVYhGecar59RAj1UwnLLEFikUySxsX
          hex!["a29a5f21c241f046d3bd4892165c9482e740660a0fd996be710124b70b390754"].into(),
          vec![
              hex!["5c4554b75c386ec90aeed9fbe8d05dbdbfd472fc087179e1965a89b29fbcb837"].into(),
              hex!["dcbb9668ae00a13b2872036fc709a217965921ef43dc04a1e8ef864095cdb64e"].into(),
              hex!["5a6506361a71bebee34c7b75ccf489a47d9d68862b8880657ddc5427e3997e2e"].into(),
              hex!["ee055ced30642570344411c86989e971fa6c2c7ccc0c15d5eeaeba07db5e1d3c"].into(),
              hex!["a4661c18c098947c16db1b4e32205e9711858fcc8e7f0ae13ca7f3888de56d03"].into(),
              hex!["b0ba7fa27e4d686270ac7c8039a67bccfd6a50d7ea5e0a17e8a0ee1a8b21ab38"].into(),
              hex!["260b44c796be109f1b9e686c9ec3339c66b8d57137c3d904b3ab11f6d0d6de79"].into(),
              hex!["5cd69fa917d797ae2d3c7c85df6520b5bc99c5cb283db569cf3306876deee666"].into(),
              hex!["441fd6b4cbfb1cc5a72c662f0f9d00d2d7d04c966204f18b7c32a1a9e573bd72"].into(),
              hex!["00aba1bf0a5efc2eb6030adfd3643fd6ac5d05a43e8170bd2e42ef7679cf1601"].into(),
          ],
          true,
          endowed_evm_account(),
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
pub fn glitch_mainnet_config() -> Result<ChainSpec, String> {
  let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
  Ok(ChainSpec::from_genesis(
      //Name
      "Glitch",
      //ID
      "glitch_mainnet",
      ChainType::Live,
      move || glitch_genesis_mainnet(
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
pub fn glitch_uat_config() -> Result<ChainSpec, String> {
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
) -> GenesisConfig {
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
        fund: Default::default(),
        revenue_fund: Default::default(),
        revenue: RevenueConfig {
            // admin_genesis: get_account_id_from_seed::<sr25519::Public>("Alice")
            admin_genesis: Some(AccountId::from_str("0x88b4fc7317577d1582969bbc2c3e179926e07c88a7507302fec5fd4f662a9567").unwrap())
        }
    }
}

fn glitch_genesis_mainnet(
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
    enable_println: bool,
    endowed_eth_accounts: BTreeMap<H160, GenesisAccount>,
) -> GenesisConfig {

  let initial_balances: Vec<(AccountId, Balance)> = vec![
    (hex!["004028fd0cf9675e2c1698c5c539f5b273e73c490cd3fb54f32923e630e66922"].into(), 8789888770000000000000000u128.into()),
    (hex!["1290c0fa454d01631a80f6b62dd080ed4e1a76c9cdef6045594dc7e2d226eb44"].into(), 8789888770000000000000000u128.into()),
    (hex!["1c282c8b1e00a3b7ba2d7a4466a23420cb398aa6e53ca8a3ec75f92cfd93e97b"].into(), 8789888770000000000000000u128.into()),
    (hex!["4e5b90d22cb365beeed1a96ffea8175e30daacb628898998810f57ec65cf7969"].into(), 8789888770000000000000000u128.into()),
    (hex!["501934c8d7b257fbadd003bb4a29a5adb9fce7fa8d659e28d473002f5fffcf65"].into(), 8789888770000000000000000u128.into()),
    (hex!["66b4c4464cfd187ae8206a29fe7079ec156d37af10d763e5171da19d66bca742"].into(), 8789888770000000000000000u128.into()),
    (hex!["9c5f9d91b99f8b1f25cf075ba57839734f3e249b72adcf04899a46c8cfd95b4e"].into(), 8789888770000000000000000u128.into()),
    (hex!["c4a01a3a57602229e112de1449d83cfbbfbc6d423da7e0ab7baeacfcf83f1d2f"].into(), 8789888770000000000000000u128.into()),
    (hex!["d8412bac516c4c079016a8ae6eefb983837274d884868dc9528e6a2a27dfbb0d"].into(), 8789888770000000000000000u128.into()),
    (hex!["1608a5e4d16f4b694a164372e1fd5af8944514d7cec9263fec457bac96e25565"].into(), 7529883938533221586000000u128.into()),
    (hex!["5c790cbdc11a4bf8934250eb27bf26f2ea05db67b4f5fa48a760bcfd9ef43b49"].into(), 330000000200000000000000u128.into()),
    (hex!["2600c75f5fe2ddb65676361769e637069cb2041622979ba118a68993279deb0b"].into(), 330000000100000000000000u128.into()),
    (hex!["68f1d67412d6528992d391025dbc36a3261c1efaf22066dff692b2e4c1fb726a"].into(), 178416986451253370000000u128.into()),
    (hex!["42db32bc3741422b85c021bddfac3a558a83528143b1be501cb8e2c87dd3136f"].into(), 139956475391367500000000u128.into()),
    (hex!["2cc43f0eb835443c7c6c01ed86095a2235ac8f53674f81ac1a71ed98187f5615"].into(), 100624997082378040000000u128.into()),
    (hex!["065647e8ae29ae3f696f1c4087852a60bfb2ba05d46c59d123b7767a90ed8c6d"].into(), 89999997082378040000000u128.into()),
    (hex!["068e505204f46e1fc793b82f64bab6d64c05e783fa93c37a147a9a527c66d174"].into(), 89999997082378040000000u128.into()),
    (hex!["0a567658e49a30fd7b4cb5055cfea38fda48f93b758aeb5090cf19a7fa3ff64e"].into(), 89999997082378040000000u128.into()),
    (hex!["407308fc2f0166f6b0b6162f1103078b453cbefb0650374b119e899b2832f368"].into(), 89999997082378040000000u128.into()),
    (hex!["4ae8af1430acada6f9d7bc7ce63ab57ac22ecd3d703438e5d365b3eff3f8da28"].into(), 89999997082378040000000u128.into()),
    (hex!["5e6c33f6942d7d02f3600ac9f22ff3f1744c32db75ee472d6aa7c36a024e6d1c"].into(), 89999997082378040000000u128.into()),
    (hex!["76799daa6ed897dfaff6df7eb2bec97e0d20f193af8d6b05725cc9413c7b7068"].into(), 89999997082378040000000u128.into()),
    (hex!["7c9be668773083c243b11beeaef3c43cdb20a3c0c4945d0abf69e15261541808"].into(), 89999997082378040000000u128.into()),
    (hex!["84f86df3cec9c1fafec26637ba3c31a5a949d7dc53c7a464648269b7da7d8310"].into(), 89999997082378040000000u128.into()),
    (hex!["8a40b984a216c8ab3b8344eb504de0d898039861adeae0469c50c3eeeaacbf51"].into(), 89999997082378040000000u128.into()),
    (hex!["a0df6d97e02ace0afee5119fe25896e70a51895b577d6250870741ab0c2d290f"].into(), 89999997082378040000000u128.into()),
    (hex!["d0e58b65a141d28ea3a302bab6c825087f9f7ed424cc26c673c7985f8fad5a41"].into(), 89999997082378040000000u128.into()),
    (hex!["f6c7b0308ab0fb7be7ec8d381814bf911fe57d2166d3cfa21d9286c4b94eaa3c"].into(), 89999997082378040000000u128.into()),
    (hex!["aaa67949d58eb167f493f3d5c6c22f9dfba5d4e183292f9809a1031a199b6702"].into(), 981996332215200000000u128.into()),
    (hex!["b2b6c5fa937df57b3adf244bf284fa10113cdc96b680c7e28e4ba86e723ad562"].into(), 11758539674242000000u128.into()),
    (hex!["d8d222c44e7b678dc07a1136ca146c8cf71d46d7327b67ab3d6abe3b4f83cb3d"].into(), 5000000000000000000u128.into()),
    (hex!["9234b3143cfcde6c6a67fc601513c54ae069e9eb1e5ff3636d3c664602844e4e"].into(), 4189464809180000000u128.into()),
    (hex!["0a276900bee9d28fa31728814afa5194cb3e6b21b55370a1ece060e5ec6d5f01"].into(), 1000000000000000000u128.into()),
    (hex!["a40aa48af6589f95ca378e1bc3eac36f4447b09c748d56409bd4968f46fc405f"].into(), 1000000000000000000u128.into()),
    (hex!["4aa44e08b891d389fb0c1d56630437830785b5ff9083c9b4e4cc0ac5793b272d"].into(), 987727686650000000u128.into()),
    (hex!["8e65621224b9e3109d4826c62ef8e349a214f4111d73457d98dc72cc6a30ed7d"].into(), 478106479712000000u128.into()),
    (hex!["6d6f646c66756e64726576650000000000000000000000000000000000000000"].into(), 100000000000000000u128.into()),
    (hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(), 100000000000000000u128.into()),
  ];

  let mut ret = glitch_genesis(wasm_binary, initial_authorities.clone(), root_key, endowed_accounts, enable_println, endowed_eth_accounts);
  ret.balances = BalancesConfig {
    // Configure endowed accounts with initial balance of 1 << 60.
    balances: initial_balances,
  };

  ret
}
