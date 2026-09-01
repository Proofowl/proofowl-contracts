// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: `stellar contract bindings typescript` run against the committed
// contract WASM (target/wasm32v1-none/release/proofowl_contracts.wasm).
// Regenerate with:  npm run generate   (or:  make sdk-generate)
// CI fails if this file drifts from a fresh regeneration.

import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




export const Errors = {
  /**
   * Reserved. Kept for numbering stability; unreachable now that
   * setup is a one-shot constructor with no `init` entrypoint.
   */
  1: {message:"AlreadyInitialized"},
  /**
   * The instance config is missing (e.g. archived). Practically
   * unreachable while the instance entry is alive — see the TTL
   * policy in `SECURITY.md`.
   */
  2: {message:"NotInitialized"},
  3: {message:"Unauthorized"},
  4: {message:"WalletAlreadyLinked"},
  5: {message:"GithubAlreadyLinked"},
  6: {message:"DuplicateAttestation"},
  7: {message:"WalletNotLinked"},
  /**
   * `complexity` was not one of the accepted tier values.
   */
  8: {message:"InvalidComplexity"},
  /**
   * `unlink_github` was called for a (wallet, github_id_hash) pair
   * that is not an existing, consistent link.
   */
  9: {message:"LinkNotFound"}
}

export type DataKey = {tag: "Admin", values: void} | {tag: "Attestor", values: void} | {tag: "WalletLink", values: readonly [string]} | {tag: "GithubLink", values: readonly [Buffer]} | {tag: "Attestations", values: readonly [string]} | {tag: "SeenPr", values: readonly [Buffer]};


/**
 * One recorded contribution.
 * 
 * `pr_hash` is the global de-duplication key and is treated as an
 * opaque 32-byte value. The backend MUST derive it canonically as:
 * 
 * ```text
 * pr_hash = SHA-256(  lowercase("github.com/<owner>/<repo>/pull/<number>")  )
 * ```
 * 
 * with no scheme, no trailing slash, and no query string, so that the
 * same PR always hashes to the same value regardless of how the URL was
 * captured. `repo` (`"<owner>/<repo>"`) and `pr_number` are stored in
 * the clear so an indexer or frontend can reconstruct that URL and link
 * straight to the pull request; `pr_hash` alone is not reversible.
 */
export interface Attestation {
  /**
 * Wave complexity tier in points. One of `0`, `100`, `150`, `200`.
 * `0` means the attestor confirmed the contribution happened but
 * could not confirm its official tier — see
 * [`ProofOwlRegistry::get_reputation_score`] for how that is scored.
 */
complexity: u32;
  /**
 * Stellar Wave issue id this contribution resolved.
 */
issue_id: u64;
  /**
 * SHA-256 of the normalized PR URL — see the type docs.
 */
pr_hash: Buffer;
  /**
 * GitHub pull-request number.
 */
pr_number: u32;
  /**
 * `"<owner>/<repo>"`, e.g. `"stellar/soroban-examples"`.
 */
repo: string;
  /**
 * Ledger timestamp (Unix seconds) at which the attestation was
 * recorded on-chain. Set by the contract, not the caller.
 */
timestamp: u64;
}






export interface Client {
  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a link_github transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Link a wallet to a hashed GitHub identity. **Two-party**: requires
   * the signatures of both `wallet` and the trusted `attestor`.
   * 
   * * The wallet signature proves control of the Stellar key.
   * * The attestor signature attests that the off-chain GitHub OAuth /
   * challenge flow proved the same person controls the GitHub
   * account behind `github_id_hash`. The contract cannot and does
   * not verify GitHub itself.
   * 
   * One wallet <-> one GitHub identity, enforced in both directions. A
   * mistaken link is undone with [`Self::unlink_github`] (also
   * two-party); there is intentionally no admin override that could
   * silently move a link.
   */
  link_github: ({wallet, attestor, github_id_hash}: {wallet: string, attestor: string, github_id_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_attestor transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_attestor: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a set_attestor transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Rotate the attestor key. Admin-only. The intended path to
   * decentralize off a single trusted key later (multisig or a
   * threshold attestor contract) without a migration.
   */
  set_attestor: ({admin, new_attestor}: {admin: string, new_attestor: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unlink_github transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Undo a link. **Two-party**: requires the signatures of both the
   * currently linked `wallet` and the trusted `attestor`. Used to fix
   * a mistaken link (wrong GitHub identity hash) or to release an
   * identity so it can be re-linked to a different wallet after the
   * owner re-runs the off-chain GitHub verification.
   * 
   * Both link records are removed. The wallet's attestation history
   * (`Attestations(wallet)`) and the global PR-dedup markers are left
   * untouched: a merged PR stays spent forever, and reputation already
   * earned stays attached to the wallet that earned it. Migrating a
   * history to a fresh wallet is out of scope for the MVP — see
   * `SECURITY.md`.
   */
  unlink_github: ({wallet, attestor, github_id_hash}: {wallet: string, attestor: string, github_id_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a bump_wallet_ttl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless keep-alive. Extends the TTL of every long-lived
   * record tied to a wallet:
   * 
   * * the wallet link and the GitHub link it points at,
   * * the attestation-history vector,
   * * **every `SeenPr` de-duplication marker** referenced by that
   * history — so a merged PR can never become re-submittable just
   * because its marker was allowed to expire.
   * 
   * Anyone can call it (a frontend "keep my passport alive" button, a
   * cron job). It only pushes out archival and never changes data.
   * No-op for an unlinked wallet with no history.
   * 
   * Cost scales with the number of attestations for the wallet — see
   * the scalability note in the module docs.
   */
  bump_wallet_ttl: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_attestations transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_attestations: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<Attestation>>>

  /**
   * Construct and simulate a submit_attestation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Record a verified contribution. Attestor-only. Resolves the wallet
   * from the on-chain GitHub link rather than trusting the caller to
   * supply one — see module docs and ADR 0001.
   * 
   * `complexity` must be one of `0`, `100`, `150`, `200`
   * ([`Error::InvalidComplexity`] otherwise). `pr_hash` is the global
   * duplicate guard and must be derived canonically — see
   * [`Attestation`]. The on-chain `timestamp` is the ledger time, not
   * a caller-supplied value.
   */
  submit_attestation: ({attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash}: {attestor: string, github_id_hash: Buffer, repo: string, pr_number: u32, issue_id: u64, complexity: u32, pr_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a get_reputation_score transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sum of complexity points across all attestations for a wallet.
   * Attestations with an unverified tier (`complexity == 0`) count at
   * [`UNVERIFIED_COMPLEXITY_SCORE`] rather than zero. The sum
   * saturates at `u32::MAX`; with the accepted tier values that ceiling
   * is unreachable in practice, and the result is fully deterministic.
   */
  get_reputation_score: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_github_for_wallet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_github_for_wallet: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Buffer>>>

  /**
   * Construct and simulate a get_wallet_for_github transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_wallet_for_github: ({github_id_hash}: {github_id_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {admin, attestor}: {admin: string, attestor: string},
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy({admin, attestor}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAACQAAAHdSZXNlcnZlZC4gS2VwdCBmb3IgbnVtYmVyaW5nIHN0YWJpbGl0eTsgdW5yZWFjaGFibGUgbm93IHRoYXQKc2V0dXAgaXMgYSBvbmUtc2hvdCBjb25zdHJ1Y3RvciB3aXRoIG5vIGBpbml0YCBlbnRyeXBvaW50LgAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAklRoZSBpbnN0YW5jZSBjb25maWcgaXMgbWlzc2luZyAoZS5nLiBhcmNoaXZlZCkuIFByYWN0aWNhbGx5CnVucmVhY2hhYmxlIHdoaWxlIHRoZSBpbnN0YW5jZSBlbnRyeSBpcyBhbGl2ZSDigJQgc2VlIHRoZSBUVEwKcG9saWN5IGluIGBTRUNVUklUWS5tZGAuAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAAAAAADFVuYXV0aG9yaXplZAAAAAMAAAAAAAAAE1dhbGxldEFscmVhZHlMaW5rZWQAAAAABAAAAAAAAAATR2l0aHViQWxyZWFkeUxpbmtlZAAAAAAFAAAAAAAAABREdXBsaWNhdGVBdHRlc3RhdGlvbgAAAAYAAAAAAAAAD1dhbGxldE5vdExpbmtlZAAAAAAHAAAANWBjb21wbGV4aXR5YCB3YXMgbm90IG9uZSBvZiB0aGUgYWNjZXB0ZWQgdGllciB2YWx1ZXMuAAAAAAAAEUludmFsaWRDb21wbGV4aXR5AAAAAAAACAAAAGhgdW5saW5rX2dpdGh1YmAgd2FzIGNhbGxlZCBmb3IgYSAod2FsbGV0LCBnaXRodWJfaWRfaGFzaCkgcGFpcgp0aGF0IGlzIG5vdCBhbiBleGlzdGluZywgY29uc2lzdGVudCBsaW5rLgAAAAxMaW5rTm90Rm91bmQAAAAJ",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABgAAAAAAAAAhQWRtaW4gYWRkcmVzcyAoaW5zdGFuY2Ugc3RvcmFnZSkuAAAAAAAABUFkbWluAAAAAAAAAAAAACRBdHRlc3RvciBhZGRyZXNzIChpbnN0YW5jZSBzdG9yYWdlKS4AAAAIQXR0ZXN0b3IAAAABAAAAG2B3YWxsZXQgLT4gZ2l0aHViX2lkX2hhc2hgLgAAAAAKV2FsbGV0TGluawAAAAAAAQAAABMAAAABAAAAG2BnaXRodWJfaWRfaGFzaCAtPiB3YWxsZXRgLgAAAAAKR2l0aHViTGluawAAAAAAAQAAA+4AAAAgAAAAAQAAAB1gd2FsbGV0IC0+IFZlYzxBdHRlc3RhdGlvbj5gLgAAAAAAAAxBdHRlc3RhdGlvbnMAAAABAAAAEwAAAAEAAAAqYHByX2hhc2ggLT4gKClgIGdsb2JhbCBkdXBsaWNhdGUtUFIgZ3VhcmQuAAAAAAAGU2VlblByAAAAAAABAAAD7gAAACA=",
        "AAAAAQAAAktPbmUgcmVjb3JkZWQgY29udHJpYnV0aW9uLgoKYHByX2hhc2hgIGlzIHRoZSBnbG9iYWwgZGUtZHVwbGljYXRpb24ga2V5IGFuZCBpcyB0cmVhdGVkIGFzIGFuCm9wYXF1ZSAzMi1ieXRlIHZhbHVlLiBUaGUgYmFja2VuZCBNVVNUIGRlcml2ZSBpdCBjYW5vbmljYWxseSBhczoKCmBgYHRleHQKcHJfaGFzaCA9IFNIQS0yNTYoICBsb3dlcmNhc2UoImdpdGh1Yi5jb20vPG93bmVyPi88cmVwbz4vcHVsbC88bnVtYmVyPiIpICApCmBgYAoKd2l0aCBubyBzY2hlbWUsIG5vIHRyYWlsaW5nIHNsYXNoLCBhbmQgbm8gcXVlcnkgc3RyaW5nLCBzbyB0aGF0IHRoZQpzYW1lIFBSIGFsd2F5cyBoYXNoZXMgdG8gdGhlIHNhbWUgdmFsdWUgcmVnYXJkbGVzcyBvZiBob3cgdGhlIFVSTCB3YXMKY2FwdHVyZWQuIGByZXBvYCAoYCI8b3duZXI+LzxyZXBvPiJgKSBhbmQgYHByX251bWJlcmAgYXJlIHN0b3JlZCBpbgp0aGUgY2xlYXIgc28gYW4gaW5kZXhlciBvciBmcm9udGVuZCBjYW4gcmVjb25zdHJ1Y3QgdGhhdCBVUkwgYW5kIGxpbmsKc3RyYWlnaHQgdG8gdGhlIHB1bGwgcmVxdWVzdDsgYHByX2hhc2hgIGFsb25lIGlzIG5vdCByZXZlcnNpYmxlLgAAAAAAAAAAC0F0dGVzdGF0aW9uAAAAAAYAAADuV2F2ZSBjb21wbGV4aXR5IHRpZXIgaW4gcG9pbnRzLiBPbmUgb2YgYDBgLCBgMTAwYCwgYDE1MGAsIGAyMDBgLgpgMGAgbWVhbnMgdGhlIGF0dGVzdG9yIGNvbmZpcm1lZCB0aGUgY29udHJpYnV0aW9uIGhhcHBlbmVkIGJ1dApjb3VsZCBub3QgY29uZmlybSBpdHMgb2ZmaWNpYWwgdGllciDigJQgc2VlCltgUHJvb2ZPd2xSZWdpc3RyeTo6Z2V0X3JlcHV0YXRpb25fc2NvcmVgXSBmb3IgaG93IHRoYXQgaXMgc2NvcmVkLgAAAAAACmNvbXBsZXhpdHkAAAAAAAQAAAAxU3RlbGxhciBXYXZlIGlzc3VlIGlkIHRoaXMgY29udHJpYnV0aW9uIHJlc29sdmVkLgAAAAAAAAhpc3N1ZV9pZAAAAAYAAAA3U0hBLTI1NiBvZiB0aGUgbm9ybWFsaXplZCBQUiBVUkwg4oCUIHNlZSB0aGUgdHlwZSBkb2NzLgAAAAAHcHJfaGFzaAAAAAPuAAAAIAAAABtHaXRIdWIgcHVsbC1yZXF1ZXN0IG51bWJlci4AAAAACXByX251bWJlcgAAAAAAAAQAAAA2YCI8b3duZXI+LzxyZXBvPiJgLCBlLmcuIGAic3RlbGxhci9zb3JvYmFuLWV4YW1wbGVzImAuAAAAAAAEcmVwbwAAABAAAAB0TGVkZ2VyIHRpbWVzdGFtcCAoVW5peCBzZWNvbmRzKSBhdCB3aGljaCB0aGUgYXR0ZXN0YXRpb24gd2FzCnJlY29yZGVkIG9uLWNoYWluLiBTZXQgYnkgdGhlIGNvbnRyYWN0LCBub3QgdGhlIGNhbGxlci4AAAAJdGltZXN0YW1wAAAAAAAABg==",
        "AAAABQAAAAAAAAAAAAAAC0luaXRpYWxpemVkAAAAAAEAAAALaW5pdGlhbGl6ZWQAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAAAAAAAAIYXR0ZXN0b3IAAAATAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADEdpdGh1YkxpbmtlZAAAAAEAAAANZ2l0aHViX2xpbmtlZAAAAAAAAAIAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAADkdpdGh1YlVubGlua2VkAAAAAAABAAAAD2dpdGh1Yl91bmxpbmtlZAAAAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAABAAAAAAAAAA5naXRodWJfaWRfaGFzaAAAAAAD7gAAACAAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAAD0F0dGVzdG9yUm90YXRlZAAAAAABAAAAEGF0dGVzdG9yX3JvdGF0ZWQAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAAAAAAxuZXdfYXR0ZXN0b3IAAAATAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAE0F0dGVzdGF0aW9uUmVjb3JkZWQAAAAAAQAAABRhdHRlc3RhdGlvbl9yZWNvcmRlZAAAAAcAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAAAAAABHJlcG8AAAAQAAAAAAAAAAAAAAAJcHJfbnVtYmVyAAAAAAAABAAAAAAAAAAAAAAACGlzc3VlX2lkAAAABgAAAAAAAAAAAAAACmNvbXBsZXhpdHkAAAAAAAQAAAAAAAAAAAAAAAdwcl9oYXNoAAAAA+4AAAAgAAAAAAAAAAAAAAAJdGltZXN0YW1wAAAAAAAABgAAAAAAAAAC",
        "AAAAAAAAAAAAAAAJZ2V0X2FkbWluAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAmNMaW5rIGEgd2FsbGV0IHRvIGEgaGFzaGVkIEdpdEh1YiBpZGVudGl0eS4gKipUd28tcGFydHkqKjogcmVxdWlyZXMKdGhlIHNpZ25hdHVyZXMgb2YgYm90aCBgd2FsbGV0YCBhbmQgdGhlIHRydXN0ZWQgYGF0dGVzdG9yYC4KCiogVGhlIHdhbGxldCBzaWduYXR1cmUgcHJvdmVzIGNvbnRyb2wgb2YgdGhlIFN0ZWxsYXIga2V5LgoqIFRoZSBhdHRlc3RvciBzaWduYXR1cmUgYXR0ZXN0cyB0aGF0IHRoZSBvZmYtY2hhaW4gR2l0SHViIE9BdXRoIC8KY2hhbGxlbmdlIGZsb3cgcHJvdmVkIHRoZSBzYW1lIHBlcnNvbiBjb250cm9scyB0aGUgR2l0SHViCmFjY291bnQgYmVoaW5kIGBnaXRodWJfaWRfaGFzaGAuIFRoZSBjb250cmFjdCBjYW5ub3QgYW5kIGRvZXMKbm90IHZlcmlmeSBHaXRIdWIgaXRzZWxmLgoKT25lIHdhbGxldCA8LT4gb25lIEdpdEh1YiBpZGVudGl0eSwgZW5mb3JjZWQgaW4gYm90aCBkaXJlY3Rpb25zLiBBCm1pc3Rha2VuIGxpbmsgaXMgdW5kb25lIHdpdGggW2BTZWxmOjp1bmxpbmtfZ2l0aHViYF0gKGFsc28KdHdvLXBhcnR5KTsgdGhlcmUgaXMgaW50ZW50aW9uYWxseSBubyBhZG1pbiBvdmVycmlkZSB0aGF0IGNvdWxkCnNpbGVudGx5IG1vdmUgYSBsaW5rLgAAAAALbGlua19naXRodWIAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAAAAAAAMZ2V0X2F0dGVzdG9yAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAKZSb3RhdGUgdGhlIGF0dGVzdG9yIGtleS4gQWRtaW4tb25seS4gVGhlIGludGVuZGVkIHBhdGggdG8KZGVjZW50cmFsaXplIG9mZiBhIHNpbmdsZSB0cnVzdGVkIGtleSBsYXRlciAobXVsdGlzaWcgb3IgYQp0aHJlc2hvbGQgYXR0ZXN0b3IgY29udHJhY3QpIHdpdGhvdXQgYSBtaWdyYXRpb24uAAAAAAAMc2V0X2F0dGVzdG9yAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAxuZXdfYXR0ZXN0b3IAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAiZEZXBsb3ktdGltZSBzZXR1cC4gVGhlIGhvc3QgY2FsbHMgdGhpcyBleGFjdGx5IG9uY2UsIGF0b21pY2FsbHksCmFzIHBhcnQgb2YgdGhlIGBDcmVhdGVDb250cmFjdGAgb3BlcmF0aW9uIHRoYXQgY3JlYXRlcyB0aGUKaW5zdGFuY2Ug4oCUIHRoZXJlIGlzIG5vIHNlcGFyYXRlIGBpbml0YCBjYWxsIGFuZCB0aGVyZWZvcmUgbm8KaW5pdGlhbGl6YXRpb24gcmFjZSB0byBmcm9udC1ydW4uCgpgYWRtaW4ucmVxdWlyZV9hdXRoKClgIG1lYW5zIHRoZSBkZXBsb3kgdHJhbnNhY3Rpb24gbXVzdCBjYXJyeSB0aGUKYWRtaW4ncyBzaWduYXR1cmUsIGJpbmRpbmcgdGhlIGNvbmZpZ3VyYXRpb24gdG8gYQpkZXBsb3llci1hdXRob3JpemVkIHNldHVwIHJhdGhlciB0aGFuIHRvIHdob2V2ZXIgY2FsbHMgZmlyc3QuCgpgYXR0ZXN0b3JgIGlzIHRoZSBrZXkgYWxsb3dlZCB0byBzdWJtaXQgYXR0ZXN0YXRpb25zIGFuZCB0bwpjby1zaWduIGlkZW50aXR5IGxpbmtzOyByb3RhdGUgaXQgbGF0ZXIgd2l0aCBbYFNlbGY6OnNldF9hdHRlc3RvcmBdCndpdGhvdXQgcmVkZXBsb3lpbmcuAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAIYXR0ZXN0b3IAAAATAAAAAA==",
        "AAAAAAAAAoNVbmRvIGEgbGluay4gKipUd28tcGFydHkqKjogcmVxdWlyZXMgdGhlIHNpZ25hdHVyZXMgb2YgYm90aCB0aGUKY3VycmVudGx5IGxpbmtlZCBgd2FsbGV0YCBhbmQgdGhlIHRydXN0ZWQgYGF0dGVzdG9yYC4gVXNlZCB0byBmaXgKYSBtaXN0YWtlbiBsaW5rICh3cm9uZyBHaXRIdWIgaWRlbnRpdHkgaGFzaCkgb3IgdG8gcmVsZWFzZSBhbgppZGVudGl0eSBzbyBpdCBjYW4gYmUgcmUtbGlua2VkIHRvIGEgZGlmZmVyZW50IHdhbGxldCBhZnRlciB0aGUKb3duZXIgcmUtcnVucyB0aGUgb2ZmLWNoYWluIEdpdEh1YiB2ZXJpZmljYXRpb24uCgpCb3RoIGxpbmsgcmVjb3JkcyBhcmUgcmVtb3ZlZC4gVGhlIHdhbGxldCdzIGF0dGVzdGF0aW9uIGhpc3RvcnkKKGBBdHRlc3RhdGlvbnMod2FsbGV0KWApIGFuZCB0aGUgZ2xvYmFsIFBSLWRlZHVwIG1hcmtlcnMgYXJlIGxlZnQKdW50b3VjaGVkOiBhIG1lcmdlZCBQUiBzdGF5cyBzcGVudCBmb3JldmVyLCBhbmQgcmVwdXRhdGlvbiBhbHJlYWR5CmVhcm5lZCBzdGF5cyBhdHRhY2hlZCB0byB0aGUgd2FsbGV0IHRoYXQgZWFybmVkIGl0LiBNaWdyYXRpbmcgYQpoaXN0b3J5IHRvIGEgZnJlc2ggd2FsbGV0IGlzIG91dCBvZiBzY29wZSBmb3IgdGhlIE1WUCDigJQgc2VlCmBTRUNVUklUWS5tZGAuAAAAAA11bmxpbmtfZ2l0aHViAAAAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAnNQZXJtaXNzaW9ubGVzcyBrZWVwLWFsaXZlLiBFeHRlbmRzIHRoZSBUVEwgb2YgZXZlcnkgbG9uZy1saXZlZApyZWNvcmQgdGllZCB0byBhIHdhbGxldDoKCiogdGhlIHdhbGxldCBsaW5rIGFuZCB0aGUgR2l0SHViIGxpbmsgaXQgcG9pbnRzIGF0LAoqIHRoZSBhdHRlc3RhdGlvbi1oaXN0b3J5IHZlY3RvciwKKiAqKmV2ZXJ5IGBTZWVuUHJgIGRlLWR1cGxpY2F0aW9uIG1hcmtlcioqIHJlZmVyZW5jZWQgYnkgdGhhdApoaXN0b3J5IOKAlCBzbyBhIG1lcmdlZCBQUiBjYW4gbmV2ZXIgYmVjb21lIHJlLXN1Ym1pdHRhYmxlIGp1c3QKYmVjYXVzZSBpdHMgbWFya2VyIHdhcyBhbGxvd2VkIHRvIGV4cGlyZS4KCkFueW9uZSBjYW4gY2FsbCBpdCAoYSBmcm9udGVuZCAia2VlcCBteSBwYXNzcG9ydCBhbGl2ZSIgYnV0dG9uLCBhCmNyb24gam9iKS4gSXQgb25seSBwdXNoZXMgb3V0IGFyY2hpdmFsIGFuZCBuZXZlciBjaGFuZ2VzIGRhdGEuCk5vLW9wIGZvciBhbiB1bmxpbmtlZCB3YWxsZXQgd2l0aCBubyBoaXN0b3J5LgoKQ29zdCBzY2FsZXMgd2l0aCB0aGUgbnVtYmVyIG9mIGF0dGVzdGF0aW9ucyBmb3IgdGhlIHdhbGxldCDigJQgc2VlCnRoZSBzY2FsYWJpbGl0eSBub3RlIGluIHRoZSBtb2R1bGUgZG9jcy4AAAAAD2J1bXBfd2FsbGV0X3R0bAAAAAABAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAQZ2V0X2F0dGVzdGF0aW9ucwAAAAEAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAPqAAAH0AAAAAtBdHRlc3RhdGlvbgA=",
        "AAAAAAAAAbtSZWNvcmQgYSB2ZXJpZmllZCBjb250cmlidXRpb24uIEF0dGVzdG9yLW9ubHkuIFJlc29sdmVzIHRoZSB3YWxsZXQKZnJvbSB0aGUgb24tY2hhaW4gR2l0SHViIGxpbmsgcmF0aGVyIHRoYW4gdHJ1c3RpbmcgdGhlIGNhbGxlciB0bwpzdXBwbHkgb25lIOKAlCBzZWUgbW9kdWxlIGRvY3MgYW5kIEFEUiAwMDAxLgoKYGNvbXBsZXhpdHlgIG11c3QgYmUgb25lIG9mIGAwYCwgYDEwMGAsIGAxNTBgLCBgMjAwYAooW2BFcnJvcjo6SW52YWxpZENvbXBsZXhpdHlgXSBvdGhlcndpc2UpLiBgcHJfaGFzaGAgaXMgdGhlIGdsb2JhbApkdXBsaWNhdGUgZ3VhcmQgYW5kIG11c3QgYmUgZGVyaXZlZCBjYW5vbmljYWxseSDigJQgc2VlCltgQXR0ZXN0YXRpb25gXS4gVGhlIG9uLWNoYWluIGB0aW1lc3RhbXBgIGlzIHRoZSBsZWRnZXIgdGltZSwgbm90CmEgY2FsbGVyLXN1cHBsaWVkIHZhbHVlLgAAAAASc3VibWl0X2F0dGVzdGF0aW9uAAAAAAAHAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAAAAAAEcmVwbwAAABAAAAAAAAAACXByX251bWJlcgAAAAAAAAQAAAAAAAAACGlzc3VlX2lkAAAABgAAAAAAAAAKY29tcGxleGl0eQAAAAAABAAAAAAAAAAHcHJfaGFzaAAAAAPuAAAAIAAAAAEAAAPpAAAAEwAAAAM=",
        "AAAAAAAAAUFTdW0gb2YgY29tcGxleGl0eSBwb2ludHMgYWNyb3NzIGFsbCBhdHRlc3RhdGlvbnMgZm9yIGEgd2FsbGV0LgpBdHRlc3RhdGlvbnMgd2l0aCBhbiB1bnZlcmlmaWVkIHRpZXIgKGBjb21wbGV4aXR5ID09IDBgKSBjb3VudCBhdApbYFVOVkVSSUZJRURfQ09NUExFWElUWV9TQ09SRWBdIHJhdGhlciB0aGFuIHplcm8uIFRoZSBzdW0Kc2F0dXJhdGVzIGF0IGB1MzI6Ok1BWGA7IHdpdGggdGhlIGFjY2VwdGVkIHRpZXIgdmFsdWVzIHRoYXQgY2VpbGluZwppcyB1bnJlYWNoYWJsZSBpbiBwcmFjdGljZSwgYW5kIHRoZSByZXN1bHQgaXMgZnVsbHkgZGV0ZXJtaW5pc3RpYy4AAAAAAAAUZ2V0X3JlcHV0YXRpb25fc2NvcmUAAAABAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAABAAAABA==",
        "AAAAAAAAAAAAAAAVZ2V0X2dpdGh1Yl9mb3Jfd2FsbGV0AAAAAAAAAQAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAQAAA+gAAAPuAAAAIA==",
        "AAAAAAAAAAAAAAAVZ2V0X3dhbGxldF9mb3JfZ2l0aHViAAAAAAAAAQAAAAAAAAAOZ2l0aHViX2lkX2hhc2gAAAAAA+4AAAAgAAAAAQAAA+gAAAAT" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_admin: this.txFromJSON<Option<string>>,
        link_github: this.txFromJSON<Result<void>>,
        get_attestor: this.txFromJSON<Option<string>>,
        set_attestor: this.txFromJSON<Result<void>>,
        unlink_github: this.txFromJSON<Result<void>>,
        bump_wallet_ttl: this.txFromJSON<null>,
        get_attestations: this.txFromJSON<Array<Attestation>>,
        submit_attestation: this.txFromJSON<Result<string>>,
        get_reputation_score: this.txFromJSON<u32>,
        get_github_for_wallet: this.txFromJSON<Option<Buffer>>,
        get_wallet_for_github: this.txFromJSON<Option<string>>
  }
}