/**
 * Contract + network configuration for the ProofOwl SDK.
 *
 * The contract id and network are ALWAYS caller-supplied. This module
 * ships one clearly-marked testnet EXAMPLE (the disposable Phase 2 alpha
 * instance); it is never a production default.
 */

/** Stellar testnet network passphrase (a well-known public constant). */
export const TESTNET_PASSPHRASE = "Test SDF Network ; September 2015";

/** Stellar mainnet network passphrase. Present only so callers can refuse it. */
export const MAINNET_PASSPHRASE = "Public Global Stellar Network ; September 2015";

export interface ProofOwlContractConfig {
  /** Deployed contract id, `C...` strkey (56 chars). */
  contractId: string;
  /** Soroban RPC endpoint. Must be `https://` unless `allowHttp` is set. */
  rpcUrl: string;
  /** Network passphrase the RPC serves. Authoritative network identifier. */
  networkPassphrase: string;
  /** Allow a plaintext `http://` RPC. Local development only. Default: false. */
  allowHttp?: boolean;
}

/**
 * EXAMPLE configuration for the disposable Phase 2 **testnet alpha**
 * instance. This is documentation, not a default: the instance may be
 * replaced by a new contract id at any time, and it holds only test
 * data. Real code must pass its own {@link ProofOwlContractConfig}.
 *
 * See `docs/testnet/phase2-alpha.md`.
 */
export const TESTNET_ALPHA_EXAMPLE: Readonly<ProofOwlContractConfig> = Object.freeze({
  contractId: "CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6",
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: TESTNET_PASSPHRASE,
});

const STRKEY_C_RE = /^C[A-Z2-7]{55}$/;

/**
 * Validate a {@link ProofOwlContractConfig}. Throws on any problem.
 * Does not touch the network.
 */
export function assertConfig(config: ProofOwlContractConfig): void {
  if (typeof config !== "object" || config === null) {
    throw new TypeError("config must be an object");
  }
  if (!STRKEY_C_RE.test(config.contractId)) {
    throw new TypeError(
      `config.contractId must be a 'C...' contract strkey, got: ${String(config.contractId)}`,
    );
  }
  if (typeof config.rpcUrl !== "string" || config.rpcUrl.length === 0) {
    throw new TypeError("config.rpcUrl must be a non-empty string");
  }
  const isHttps = config.rpcUrl.startsWith("https://");
  const isHttp = config.rpcUrl.startsWith("http://");
  if (!isHttps && !(isHttp && config.allowHttp === true)) {
    throw new TypeError(
      "config.rpcUrl must be https:// (or http:// with allowHttp: true for local dev)",
    );
  }
  if (typeof config.networkPassphrase !== "string" || config.networkPassphrase.length === 0) {
    throw new TypeError("config.networkPassphrase must be a non-empty string");
  }
}

/** True if the config targets Stellar mainnet (by passphrase). */
export function isMainnet(config: ProofOwlContractConfig): boolean {
  return config.networkPassphrase === MAINNET_PASSPHRASE;
}
