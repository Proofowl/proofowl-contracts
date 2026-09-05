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
  9: {message:"LinkNotFound"},
  /**
   * A paginated call's `limit` was `0`. v0.2.
   */
  10: {message:"InvalidPageLimit"},
  /**
   * A paginated call's `limit` exceeded [`MAX_PAGE_SIZE`]. v0.2.
   */
  11: {message:"PageLimitExceeded"},
  /**
   * [`ProofOwlRegistry::get_attestation`]'s `sequence` was `>=` the
   * wallet's attestation count. v0.2.
   */
  12: {message:"SequenceOutOfRange"},
  /**
   * A paginated call's `start` was `>` the wallet's attestation
   * count. `start == count` is valid (yields an empty page) — see
   * [`ProofOwlRegistry::get_attestations_page`]. v0.2.
   */
  13: {message:"PageStartOutOfRange"}
}

export type DataKey = {tag: "Admin", values: void} | {tag: "Attestor", values: void} | {tag: "WalletLink", values: readonly [string]} | {tag: "GithubLink", values: readonly [Buffer]} | {tag: "AttestationEntry", values: readonly [string, u32]} | {tag: "AttestationCount", values: readonly [string]} | {tag: "ReputationScore", values: readonly [string]} | {tag: "SeenPr", values: readonly [Buffer]};


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
   * and the global PR-dedup markers are left untouched: a merged PR
   * stays spent forever, and reputation already earned stays attached
   * to the wallet that earned it. Migrating a history to a fresh
   * wallet is out of scope — see `SECURITY.md`.
   */
  unlink_github: ({wallet, attestor, github_id_hash}: {wallet: string, attestor: string, github_id_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_attestation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * A single attestation by its zero-based `sequence` (`0` = the
   * first ever recorded for `wallet`). [`Error::SequenceOutOfRange`]
   * if `sequence >= get_attestation_count(wallet)`.
   */
  get_attestation: ({wallet, sequence}: {wallet: string, sequence: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Attestation>>>

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
   * 
   * Stored as one new persistent entry
   * (`AttestationEntry(wallet, seq)`) rather than appended to a
   * growing vector — see
   * `docs/adr/0004-paginated-attestation-storage.md`.
   */
  submit_attestation: ({attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash}: {attestor: string, github_id_hash: Buffer, repo: string, pr_number: u32, issue_id: u64, complexity: u32, pr_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a bump_wallet_core_ttl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless keep-alive for the O(1) records tied to a wallet:
   * the wallet link, the GitHub link it points at, the attestation
   * counter, and the reputation score. Does **not** touch individual
   * attestation entries or their `SeenPr` markers — those are swept
   * separately, in bounded pages, by
   * [`Self::bump_attestations_ttl_page`].
   * 
   * Anyone can call it (a frontend "keep my passport alive" button, a
   * cron job). It only pushes out archival and never changes data.
   * No-op for a wallet with no link and no history. Cost is constant
   * regardless of history size — see
   * `docs/adr/0004-paginated-attestation-storage.md`.
   */
  bump_wallet_core_ttl: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_reputation_score transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sum of complexity points across all attestations for a wallet.
   * Attestations with an unverified tier (`complexity == 0`) count at
   * [`UNVERIFIED_COMPLEXITY_SCORE`] rather than zero. The sum
   * saturates at `u32::MAX`; with the accepted tier values that ceiling
   * is unreachable in practice, and the result is fully deterministic.
   * 
   * O(1): reads the running counter `submit_attestation` maintains
   * atomically, never re-derived from a scan of the history — see
   * `docs/adr/0004-paginated-attestation-storage.md`.
   */
  get_reputation_score: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_attestation_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * How many attestations `wallet` has. `0` for an unknown or
   * never-attested wallet. Also the next `sequence`
   * [`Self::get_attestation`] will return once one more attestation
   * is submitted.
   */
  get_attestation_count: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_attestations_page transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * A bounded page of `wallet`'s attestation history, oldest first,
   * starting at zero-based index `start`.
   * 
   * `limit` must be in `1..=`[`MAX_PAGE_SIZE`]
   * ([`Error::InvalidPageLimit`] for `0`, [`Error::PageLimitExceeded`]
   * above the maximum). `start` must be `<=` the wallet's attestation
   * count ([`Error::PageStartOutOfRange`] otherwise) — `start` equal
   * to the count is valid and returns an empty page, the normal
   * "no more pages" signal for a caller iterating to the end.
   * 
   * Replaces v0.1's unbounded `get_attestations`: this call's cost
   * and response size are bounded by `limit` regardless of how large
   * `wallet`'s total history is — see
   * `docs/adr/0004-paginated-attestation-storage.md`.
   */
  get_attestations_page: ({wallet, start, limit}: {wallet: string, start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<Attestation>>>>

  /**
   * Construct and simulate a get_github_for_wallet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_github_for_wallet: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Buffer>>>

  /**
   * Construct and simulate a get_wallet_for_github transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_wallet_for_github: ({github_id_hash}: {github_id_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a bump_attestations_ttl_page transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless, bounded keep-alive for one page of a wallet's
   * attestation history: extends the TTL of each
   * `AttestationEntry(wallet, seq)` in `[start, start+limit)` and the
   * `SeenPr` marker its `pr_hash` points at, so a merged PR can never
   * become re-submittable just because its marker was allowed to
   * expire — regardless of which page it happens to be on.
   * 
   * Same `limit`/`start` rules as
   * [`Self::get_attestations_page`] ([`Error::InvalidPageLimit`],
   * [`Error::PageLimitExceeded`], [`Error::PageStartOutOfRange`]).
   * Returns the number of entries actually refreshed: a caller
   * sweeping a wallet's full history calls this repeatedly with an
   * advancing `start`, and a return value less than `limit`
   * (including `0`) signals the sweep has reached the end. Changes no
   * data — see `docs/security/resource-profile-v2.md` for the backend
   * scheduling responsibility this implies, and
   * `docs/adr/0004-paginated-attestation-storage.md` for why this is
   * a separate call from [`Self::bump_wallet_core_ttl`].
   */
  bump_attestations_ttl_page: ({wallet, start, limit}: {wallet: string, start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAADQAAAHdSZXNlcnZlZC4gS2VwdCBmb3IgbnVtYmVyaW5nIHN0YWJpbGl0eTsgdW5yZWFjaGFibGUgbm93IHRoYXQKc2V0dXAgaXMgYSBvbmUtc2hvdCBjb25zdHJ1Y3RvciB3aXRoIG5vIGBpbml0YCBlbnRyeXBvaW50LgAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAklRoZSBpbnN0YW5jZSBjb25maWcgaXMgbWlzc2luZyAoZS5nLiBhcmNoaXZlZCkuIFByYWN0aWNhbGx5CnVucmVhY2hhYmxlIHdoaWxlIHRoZSBpbnN0YW5jZSBlbnRyeSBpcyBhbGl2ZSDigJQgc2VlIHRoZSBUVEwKcG9saWN5IGluIGBTRUNVUklUWS5tZGAuAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAAAAAADFVuYXV0aG9yaXplZAAAAAMAAAAAAAAAE1dhbGxldEFscmVhZHlMaW5rZWQAAAAABAAAAAAAAAATR2l0aHViQWxyZWFkeUxpbmtlZAAAAAAFAAAAAAAAABREdXBsaWNhdGVBdHRlc3RhdGlvbgAAAAYAAAAAAAAAD1dhbGxldE5vdExpbmtlZAAAAAAHAAAANWBjb21wbGV4aXR5YCB3YXMgbm90IG9uZSBvZiB0aGUgYWNjZXB0ZWQgdGllciB2YWx1ZXMuAAAAAAAAEUludmFsaWRDb21wbGV4aXR5AAAAAAAACAAAAGhgdW5saW5rX2dpdGh1YmAgd2FzIGNhbGxlZCBmb3IgYSAod2FsbGV0LCBnaXRodWJfaWRfaGFzaCkgcGFpcgp0aGF0IGlzIG5vdCBhbiBleGlzdGluZywgY29uc2lzdGVudCBsaW5rLgAAAAxMaW5rTm90Rm91bmQAAAAJAAAAKUEgcGFnaW5hdGVkIGNhbGwncyBgbGltaXRgIHdhcyBgMGAuIHYwLjIuAAAAAAAAEEludmFsaWRQYWdlTGltaXQAAAAKAAAAPEEgcGFnaW5hdGVkIGNhbGwncyBgbGltaXRgIGV4Y2VlZGVkIFtgTUFYX1BBR0VfU0laRWBdLiB2MC4yLgAAABFQYWdlTGltaXRFeGNlZWRlZAAAAAAAAAsAAABhW2BQcm9vZk93bFJlZ2lzdHJ5OjpnZXRfYXR0ZXN0YXRpb25gXSdzIGBzZXF1ZW5jZWAgd2FzIGA+PWAgdGhlCndhbGxldCdzIGF0dGVzdGF0aW9uIGNvdW50LiB2MC4yLgAAAAAAABJTZXF1ZW5jZU91dE9mUmFuZ2UAAAAAAAwAAACuQSBwYWdpbmF0ZWQgY2FsbCdzIGBzdGFydGAgd2FzIGA+YCB0aGUgd2FsbGV0J3MgYXR0ZXN0YXRpb24KY291bnQuIGBzdGFydCA9PSBjb3VudGAgaXMgdmFsaWQgKHlpZWxkcyBhbiBlbXB0eSBwYWdlKSDigJQgc2VlCltgUHJvb2ZPd2xSZWdpc3RyeTo6Z2V0X2F0dGVzdGF0aW9uc19wYWdlYF0uIHYwLjIuAAAAAAATUGFnZVN0YXJ0T3V0T2ZSYW5nZQAAAAAN",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAACAAAAAAAAAAhQWRtaW4gYWRkcmVzcyAoaW5zdGFuY2Ugc3RvcmFnZSkuAAAAAAAABUFkbWluAAAAAAAAAAAAACRBdHRlc3RvciBhZGRyZXNzIChpbnN0YW5jZSBzdG9yYWdlKS4AAAAIQXR0ZXN0b3IAAAABAAAAG2B3YWxsZXQgLT4gZ2l0aHViX2lkX2hhc2hgLgAAAAAKV2FsbGV0TGluawAAAAAAAQAAABMAAAABAAAAG2BnaXRodWJfaWRfaGFzaCAtPiB3YWxsZXRgLgAAAAAKR2l0aHViTGluawAAAAAAAQAAA+4AAAAgAAAAAQAAAJpgKHdhbGxldCwgc2VxdWVuY2UpIC0+IEF0dGVzdGF0aW9uYC4gT25lIHBlcnNpc3RlbnQgZW50cnkgcGVyCmF0dGVzdGF0aW9uLCBgc2VxdWVuY2VgIHplcm8tYmFzZWQg4oCUIHNlZQpgZG9jcy9hZHIvMDAwNC1wYWdpbmF0ZWQtYXR0ZXN0YXRpb24tc3RvcmFnZS5tZGAuAAAAAAAQQXR0ZXN0YXRpb25FbnRyeQAAAAIAAAATAAAABAAAAAEAAABcYHdhbGxldCAtPiB1MzJgOiBob3cgbWFueSBhdHRlc3RhdGlvbnMgdGhpcyB3YWxsZXQgaGFzLCBhbmQgdGhlCm5leHQgYHNlcXVlbmNlYCB0byB3cml0ZSBhdC4AAAAQQXR0ZXN0YXRpb25Db3VudAAAAAEAAAATAAAAAQAAALJgd2FsbGV0IC0+IHUzMmA6IHJ1bm5pbmcgcmVwdXRhdGlvbiBzY29yZSwgdXBkYXRlZCBhdG9taWNhbGx5IGJ5CmBzdWJtaXRfYXR0ZXN0YXRpb25gLiBOZXZlciByZS1kZXJpdmVkIGZyb20gYSBmdWxsIHNjYW4g4oCUIHNlZQpgZG9jcy9hZHIvMDAwNC1wYWdpbmF0ZWQtYXR0ZXN0YXRpb24tc3RvcmFnZS5tZGAuAAAAAAAPUmVwdXRhdGlvblNjb3JlAAAAAAEAAAATAAAAAQAAACpgcHJfaGFzaCAtPiAoKWAgZ2xvYmFsIGR1cGxpY2F0ZS1QUiBndWFyZC4AAAAAAAZTZWVuUHIAAAAAAAEAAAPuAAAAIA==",
        "AAAAAQAAAktPbmUgcmVjb3JkZWQgY29udHJpYnV0aW9uLgoKYHByX2hhc2hgIGlzIHRoZSBnbG9iYWwgZGUtZHVwbGljYXRpb24ga2V5IGFuZCBpcyB0cmVhdGVkIGFzIGFuCm9wYXF1ZSAzMi1ieXRlIHZhbHVlLiBUaGUgYmFja2VuZCBNVVNUIGRlcml2ZSBpdCBjYW5vbmljYWxseSBhczoKCmBgYHRleHQKcHJfaGFzaCA9IFNIQS0yNTYoICBsb3dlcmNhc2UoImdpdGh1Yi5jb20vPG93bmVyPi88cmVwbz4vcHVsbC88bnVtYmVyPiIpICApCmBgYAoKd2l0aCBubyBzY2hlbWUsIG5vIHRyYWlsaW5nIHNsYXNoLCBhbmQgbm8gcXVlcnkgc3RyaW5nLCBzbyB0aGF0IHRoZQpzYW1lIFBSIGFsd2F5cyBoYXNoZXMgdG8gdGhlIHNhbWUgdmFsdWUgcmVnYXJkbGVzcyBvZiBob3cgdGhlIFVSTCB3YXMKY2FwdHVyZWQuIGByZXBvYCAoYCI8b3duZXI+LzxyZXBvPiJgKSBhbmQgYHByX251bWJlcmAgYXJlIHN0b3JlZCBpbgp0aGUgY2xlYXIgc28gYW4gaW5kZXhlciBvciBmcm9udGVuZCBjYW4gcmVjb25zdHJ1Y3QgdGhhdCBVUkwgYW5kIGxpbmsKc3RyYWlnaHQgdG8gdGhlIHB1bGwgcmVxdWVzdDsgYHByX2hhc2hgIGFsb25lIGlzIG5vdCByZXZlcnNpYmxlLgAAAAAAAAAAC0F0dGVzdGF0aW9uAAAAAAYAAADuV2F2ZSBjb21wbGV4aXR5IHRpZXIgaW4gcG9pbnRzLiBPbmUgb2YgYDBgLCBgMTAwYCwgYDE1MGAsIGAyMDBgLgpgMGAgbWVhbnMgdGhlIGF0dGVzdG9yIGNvbmZpcm1lZCB0aGUgY29udHJpYnV0aW9uIGhhcHBlbmVkIGJ1dApjb3VsZCBub3QgY29uZmlybSBpdHMgb2ZmaWNpYWwgdGllciDigJQgc2VlCltgUHJvb2ZPd2xSZWdpc3RyeTo6Z2V0X3JlcHV0YXRpb25fc2NvcmVgXSBmb3IgaG93IHRoYXQgaXMgc2NvcmVkLgAAAAAACmNvbXBsZXhpdHkAAAAAAAQAAAAxU3RlbGxhciBXYXZlIGlzc3VlIGlkIHRoaXMgY29udHJpYnV0aW9uIHJlc29sdmVkLgAAAAAAAAhpc3N1ZV9pZAAAAAYAAAA3U0hBLTI1NiBvZiB0aGUgbm9ybWFsaXplZCBQUiBVUkwg4oCUIHNlZSB0aGUgdHlwZSBkb2NzLgAAAAAHcHJfaGFzaAAAAAPuAAAAIAAAABtHaXRIdWIgcHVsbC1yZXF1ZXN0IG51bWJlci4AAAAACXByX251bWJlcgAAAAAAAAQAAAA2YCI8b3duZXI+LzxyZXBvPiJgLCBlLmcuIGAic3RlbGxhci9zb3JvYmFuLWV4YW1wbGVzImAuAAAAAAAEcmVwbwAAABAAAAB0TGVkZ2VyIHRpbWVzdGFtcCAoVW5peCBzZWNvbmRzKSBhdCB3aGljaCB0aGUgYXR0ZXN0YXRpb24gd2FzCnJlY29yZGVkIG9uLWNoYWluLiBTZXQgYnkgdGhlIGNvbnRyYWN0LCBub3QgdGhlIGNhbGxlci4AAAAJdGltZXN0YW1wAAAAAAAABg==",
        "AAAABQAAAAAAAAAAAAAAC0luaXRpYWxpemVkAAAAAAEAAAALaW5pdGlhbGl6ZWQAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAAAAAAAAIYXR0ZXN0b3IAAAATAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADEdpdGh1YkxpbmtlZAAAAAEAAAANZ2l0aHViX2xpbmtlZAAAAAAAAAIAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAADkdpdGh1YlVubGlua2VkAAAAAAABAAAAD2dpdGh1Yl91bmxpbmtlZAAAAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAABAAAAAAAAAA5naXRodWJfaWRfaGFzaAAAAAAD7gAAACAAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAAD0F0dGVzdG9yUm90YXRlZAAAAAABAAAAEGF0dGVzdG9yX3JvdGF0ZWQAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAAAAAAxuZXdfYXR0ZXN0b3IAAAATAAAAAAAAAAI=",
        "AAAAAAAAAAAAAAAJZ2V0X2FkbWluAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAABQAAAAAAAAAAAAAAE0F0dGVzdGF0aW9uUmVjb3JkZWQAAAAAAQAAABRhdHRlc3RhdGlvbl9yZWNvcmRlZAAAAAgAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAAAAAABHJlcG8AAAAQAAAAAAAAAAAAAAAJcHJfbnVtYmVyAAAAAAAABAAAAAAAAAAAAAAACGlzc3VlX2lkAAAABgAAAAAAAAAAAAAACmNvbXBsZXhpdHkAAAAAAAQAAAAAAAAAAAAAAAdwcl9oYXNoAAAAA+4AAAAgAAAAAAAAAAAAAAAJdGltZXN0YW1wAAAAAAAABgAAAAAAAAGBWmVyby1iYXNlZCBpbmRleCBvZiB0aGlzIGF0dGVzdGF0aW9uIGluIGB3YWxsZXRgJ3MgaGlzdG9yeSDigJQKdGhlIHNhbWUgYHNlcXVlbmNlYCBbYFByb29mT3dsUmVnaXN0cnk6OmdldF9hdHRlc3RhdGlvbmBdIGFuZApbYFByb29mT3dsUmVnaXN0cnk6OmdldF9hdHRlc3RhdGlvbnNfcGFnZWBdIGFkZHJlc3MgaXQgYnkuIE5ldwppbiB2MC4yOyBhbiBpbmRleGVyIGJ1aWxkaW5nIGEgcGFzc3BvcnQgZnJvbSBldmVudHMgYWxvbmUgY2FuCnVzZSBpdCB0byBkZXRlY3QgZ2FwcyBvciByZW9yZGVyaW5ncyB3aXRob3V0IGEgc2VwYXJhdGUKYGdldF9hdHRlc3RhdGlvbl9jb3VudGAgcm91bmQtdHJpcC4gU2VlCmBkb2NzL2ludGVncmF0aW9uL2V2ZW50LWluZGV4ZXItdjIubWRgLgAAAAAAAAhzZXF1ZW5jZQAAAAQAAAAAAAAAAg==",
        "AAAAAAAAAmNMaW5rIGEgd2FsbGV0IHRvIGEgaGFzaGVkIEdpdEh1YiBpZGVudGl0eS4gKipUd28tcGFydHkqKjogcmVxdWlyZXMKdGhlIHNpZ25hdHVyZXMgb2YgYm90aCBgd2FsbGV0YCBhbmQgdGhlIHRydXN0ZWQgYGF0dGVzdG9yYC4KCiogVGhlIHdhbGxldCBzaWduYXR1cmUgcHJvdmVzIGNvbnRyb2wgb2YgdGhlIFN0ZWxsYXIga2V5LgoqIFRoZSBhdHRlc3RvciBzaWduYXR1cmUgYXR0ZXN0cyB0aGF0IHRoZSBvZmYtY2hhaW4gR2l0SHViIE9BdXRoIC8KY2hhbGxlbmdlIGZsb3cgcHJvdmVkIHRoZSBzYW1lIHBlcnNvbiBjb250cm9scyB0aGUgR2l0SHViCmFjY291bnQgYmVoaW5kIGBnaXRodWJfaWRfaGFzaGAuIFRoZSBjb250cmFjdCBjYW5ub3QgYW5kIGRvZXMKbm90IHZlcmlmeSBHaXRIdWIgaXRzZWxmLgoKT25lIHdhbGxldCA8LT4gb25lIEdpdEh1YiBpZGVudGl0eSwgZW5mb3JjZWQgaW4gYm90aCBkaXJlY3Rpb25zLiBBCm1pc3Rha2VuIGxpbmsgaXMgdW5kb25lIHdpdGggW2BTZWxmOjp1bmxpbmtfZ2l0aHViYF0gKGFsc28KdHdvLXBhcnR5KTsgdGhlcmUgaXMgaW50ZW50aW9uYWxseSBubyBhZG1pbiBvdmVycmlkZSB0aGF0IGNvdWxkCnNpbGVudGx5IG1vdmUgYSBsaW5rLgAAAAALbGlua19naXRodWIAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAAAAAAAMZ2V0X2F0dGVzdG9yAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAKZSb3RhdGUgdGhlIGF0dGVzdG9yIGtleS4gQWRtaW4tb25seS4gVGhlIGludGVuZGVkIHBhdGggdG8KZGVjZW50cmFsaXplIG9mZiBhIHNpbmdsZSB0cnVzdGVkIGtleSBsYXRlciAobXVsdGlzaWcgb3IgYQp0aHJlc2hvbGQgYXR0ZXN0b3IgY29udHJhY3QpIHdpdGhvdXQgYSBtaWdyYXRpb24uAAAAAAAMc2V0X2F0dGVzdG9yAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAxuZXdfYXR0ZXN0b3IAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAiZEZXBsb3ktdGltZSBzZXR1cC4gVGhlIGhvc3QgY2FsbHMgdGhpcyBleGFjdGx5IG9uY2UsIGF0b21pY2FsbHksCmFzIHBhcnQgb2YgdGhlIGBDcmVhdGVDb250cmFjdGAgb3BlcmF0aW9uIHRoYXQgY3JlYXRlcyB0aGUKaW5zdGFuY2Ug4oCUIHRoZXJlIGlzIG5vIHNlcGFyYXRlIGBpbml0YCBjYWxsIGFuZCB0aGVyZWZvcmUgbm8KaW5pdGlhbGl6YXRpb24gcmFjZSB0byBmcm9udC1ydW4uCgpgYWRtaW4ucmVxdWlyZV9hdXRoKClgIG1lYW5zIHRoZSBkZXBsb3kgdHJhbnNhY3Rpb24gbXVzdCBjYXJyeSB0aGUKYWRtaW4ncyBzaWduYXR1cmUsIGJpbmRpbmcgdGhlIGNvbmZpZ3VyYXRpb24gdG8gYQpkZXBsb3llci1hdXRob3JpemVkIHNldHVwIHJhdGhlciB0aGFuIHRvIHdob2V2ZXIgY2FsbHMgZmlyc3QuCgpgYXR0ZXN0b3JgIGlzIHRoZSBrZXkgYWxsb3dlZCB0byBzdWJtaXQgYXR0ZXN0YXRpb25zIGFuZCB0bwpjby1zaWduIGlkZW50aXR5IGxpbmtzOyByb3RhdGUgaXQgbGF0ZXIgd2l0aCBbYFNlbGY6OnNldF9hdHRlc3RvcmBdCndpdGhvdXQgcmVkZXBsb3lpbmcuAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAIYXR0ZXN0b3IAAAATAAAAAA==",
        "AAAAAAAAAl5VbmRvIGEgbGluay4gKipUd28tcGFydHkqKjogcmVxdWlyZXMgdGhlIHNpZ25hdHVyZXMgb2YgYm90aCB0aGUKY3VycmVudGx5IGxpbmtlZCBgd2FsbGV0YCBhbmQgdGhlIHRydXN0ZWQgYGF0dGVzdG9yYC4gVXNlZCB0byBmaXgKYSBtaXN0YWtlbiBsaW5rICh3cm9uZyBHaXRIdWIgaWRlbnRpdHkgaGFzaCkgb3IgdG8gcmVsZWFzZSBhbgppZGVudGl0eSBzbyBpdCBjYW4gYmUgcmUtbGlua2VkIHRvIGEgZGlmZmVyZW50IHdhbGxldCBhZnRlciB0aGUKb3duZXIgcmUtcnVucyB0aGUgb2ZmLWNoYWluIEdpdEh1YiB2ZXJpZmljYXRpb24uCgpCb3RoIGxpbmsgcmVjb3JkcyBhcmUgcmVtb3ZlZC4gVGhlIHdhbGxldCdzIGF0dGVzdGF0aW9uIGhpc3RvcnkKYW5kIHRoZSBnbG9iYWwgUFItZGVkdXAgbWFya2VycyBhcmUgbGVmdCB1bnRvdWNoZWQ6IGEgbWVyZ2VkIFBSCnN0YXlzIHNwZW50IGZvcmV2ZXIsIGFuZCByZXB1dGF0aW9uIGFscmVhZHkgZWFybmVkIHN0YXlzIGF0dGFjaGVkCnRvIHRoZSB3YWxsZXQgdGhhdCBlYXJuZWQgaXQuIE1pZ3JhdGluZyBhIGhpc3RvcnkgdG8gYSBmcmVzaAp3YWxsZXQgaXMgb3V0IG9mIHNjb3BlIOKAlCBzZWUgYFNFQ1VSSVRZLm1kYC4AAAAAAA11bmxpbmtfZ2l0aHViAAAAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAK1BIHNpbmdsZSBhdHRlc3RhdGlvbiBieSBpdHMgemVyby1iYXNlZCBgc2VxdWVuY2VgIChgMGAgPSB0aGUKZmlyc3QgZXZlciByZWNvcmRlZCBmb3IgYHdhbGxldGApLiBbYEVycm9yOjpTZXF1ZW5jZU91dE9mUmFuZ2VgXQppZiBgc2VxdWVuY2UgPj0gZ2V0X2F0dGVzdGF0aW9uX2NvdW50KHdhbGxldClgLgAAAAAAAA9nZXRfYXR0ZXN0YXRpb24AAAAAAgAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAhzZXF1ZW5jZQAAAAQAAAABAAAD6QAAB9AAAAALQXR0ZXN0YXRpb24AAAAAAw==",
        "AAAAAAAAAmRSZWNvcmQgYSB2ZXJpZmllZCBjb250cmlidXRpb24uIEF0dGVzdG9yLW9ubHkuIFJlc29sdmVzIHRoZSB3YWxsZXQKZnJvbSB0aGUgb24tY2hhaW4gR2l0SHViIGxpbmsgcmF0aGVyIHRoYW4gdHJ1c3RpbmcgdGhlIGNhbGxlciB0bwpzdXBwbHkgb25lIOKAlCBzZWUgbW9kdWxlIGRvY3MgYW5kIEFEUiAwMDAxLgoKYGNvbXBsZXhpdHlgIG11c3QgYmUgb25lIG9mIGAwYCwgYDEwMGAsIGAxNTBgLCBgMjAwYAooW2BFcnJvcjo6SW52YWxpZENvbXBsZXhpdHlgXSBvdGhlcndpc2UpLiBgcHJfaGFzaGAgaXMgdGhlIGdsb2JhbApkdXBsaWNhdGUgZ3VhcmQgYW5kIG11c3QgYmUgZGVyaXZlZCBjYW5vbmljYWxseSDigJQgc2VlCltgQXR0ZXN0YXRpb25gXS4gVGhlIG9uLWNoYWluIGB0aW1lc3RhbXBgIGlzIHRoZSBsZWRnZXIgdGltZSwgbm90CmEgY2FsbGVyLXN1cHBsaWVkIHZhbHVlLgoKU3RvcmVkIGFzIG9uZSBuZXcgcGVyc2lzdGVudCBlbnRyeQooYEF0dGVzdGF0aW9uRW50cnkod2FsbGV0LCBzZXEpYCkgcmF0aGVyIHRoYW4gYXBwZW5kZWQgdG8gYQpncm93aW5nIHZlY3RvciDigJQgc2VlCmBkb2NzL2Fkci8wMDA0LXBhZ2luYXRlZC1hdHRlc3RhdGlvbi1zdG9yYWdlLm1kYC4AAAASc3VibWl0X2F0dGVzdGF0aW9uAAAAAAAHAAAAAAAAAAhhdHRlc3RvcgAAABMAAAAAAAAADmdpdGh1Yl9pZF9oYXNoAAAAAAPuAAAAIAAAAAAAAAAEcmVwbwAAABAAAAAAAAAACXByX251bWJlcgAAAAAAAAQAAAAAAAAACGlzc3VlX2lkAAAABgAAAAAAAAAKY29tcGxleGl0eQAAAAAABAAAAAAAAAAHcHJfaGFzaAAAAAPuAAAAIAAAAAEAAAPpAAAAEwAAAAM=",
        "AAAAAAAAAmFQZXJtaXNzaW9ubGVzcyBrZWVwLWFsaXZlIGZvciB0aGUgTygxKSByZWNvcmRzIHRpZWQgdG8gYSB3YWxsZXQ6CnRoZSB3YWxsZXQgbGluaywgdGhlIEdpdEh1YiBsaW5rIGl0IHBvaW50cyBhdCwgdGhlIGF0dGVzdGF0aW9uCmNvdW50ZXIsIGFuZCB0aGUgcmVwdXRhdGlvbiBzY29yZS4gRG9lcyAqKm5vdCoqIHRvdWNoIGluZGl2aWR1YWwKYXR0ZXN0YXRpb24gZW50cmllcyBvciB0aGVpciBgU2VlblByYCBtYXJrZXJzIOKAlCB0aG9zZSBhcmUgc3dlcHQKc2VwYXJhdGVseSwgaW4gYm91bmRlZCBwYWdlcywgYnkKW2BTZWxmOjpidW1wX2F0dGVzdGF0aW9uc190dGxfcGFnZWBdLgoKQW55b25lIGNhbiBjYWxsIGl0IChhIGZyb250ZW5kICJrZWVwIG15IHBhc3Nwb3J0IGFsaXZlIiBidXR0b24sIGEKY3JvbiBqb2IpLiBJdCBvbmx5IHB1c2hlcyBvdXQgYXJjaGl2YWwgYW5kIG5ldmVyIGNoYW5nZXMgZGF0YS4KTm8tb3AgZm9yIGEgd2FsbGV0IHdpdGggbm8gbGluayBhbmQgbm8gaGlzdG9yeS4gQ29zdCBpcyBjb25zdGFudApyZWdhcmRsZXNzIG9mIGhpc3Rvcnkgc2l6ZSDigJQgc2VlCmBkb2NzL2Fkci8wMDA0LXBhZ2luYXRlZC1hdHRlc3RhdGlvbi1zdG9yYWdlLm1kYC4AAAAAAAAUYnVtcF93YWxsZXRfY29yZV90dGwAAAABAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAA",
        "AAAAAAAAAfNTdW0gb2YgY29tcGxleGl0eSBwb2ludHMgYWNyb3NzIGFsbCBhdHRlc3RhdGlvbnMgZm9yIGEgd2FsbGV0LgpBdHRlc3RhdGlvbnMgd2l0aCBhbiB1bnZlcmlmaWVkIHRpZXIgKGBjb21wbGV4aXR5ID09IDBgKSBjb3VudCBhdApbYFVOVkVSSUZJRURfQ09NUExFWElUWV9TQ09SRWBdIHJhdGhlciB0aGFuIHplcm8uIFRoZSBzdW0Kc2F0dXJhdGVzIGF0IGB1MzI6Ok1BWGA7IHdpdGggdGhlIGFjY2VwdGVkIHRpZXIgdmFsdWVzIHRoYXQgY2VpbGluZwppcyB1bnJlYWNoYWJsZSBpbiBwcmFjdGljZSwgYW5kIHRoZSByZXN1bHQgaXMgZnVsbHkgZGV0ZXJtaW5pc3RpYy4KCk8oMSk6IHJlYWRzIHRoZSBydW5uaW5nIGNvdW50ZXIgYHN1Ym1pdF9hdHRlc3RhdGlvbmAgbWFpbnRhaW5zCmF0b21pY2FsbHksIG5ldmVyIHJlLWRlcml2ZWQgZnJvbSBhIHNjYW4gb2YgdGhlIGhpc3Rvcnkg4oCUIHNlZQpgZG9jcy9hZHIvMDAwNC1wYWdpbmF0ZWQtYXR0ZXN0YXRpb24tc3RvcmFnZS5tZGAuAAAAABRnZXRfcmVwdXRhdGlvbl9zY29yZQAAAAEAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAE",
        "AAAAAAAAALdIb3cgbWFueSBhdHRlc3RhdGlvbnMgYHdhbGxldGAgaGFzLiBgMGAgZm9yIGFuIHVua25vd24gb3IKbmV2ZXItYXR0ZXN0ZWQgd2FsbGV0LiBBbHNvIHRoZSBuZXh0IGBzZXF1ZW5jZWAKW2BTZWxmOjpnZXRfYXR0ZXN0YXRpb25gXSB3aWxsIHJldHVybiBvbmNlIG9uZSBtb3JlIGF0dGVzdGF0aW9uCmlzIHN1Ym1pdHRlZC4AAAAAFWdldF9hdHRlc3RhdGlvbl9jb3VudAAAAAAAAAEAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAAE",
        "AAAAAAAAAqZBIGJvdW5kZWQgcGFnZSBvZiBgd2FsbGV0YCdzIGF0dGVzdGF0aW9uIGhpc3RvcnksIG9sZGVzdCBmaXJzdCwKc3RhcnRpbmcgYXQgemVyby1iYXNlZCBpbmRleCBgc3RhcnRgLgoKYGxpbWl0YCBtdXN0IGJlIGluIGAxLi49YFtgTUFYX1BBR0VfU0laRWBdCihbYEVycm9yOjpJbnZhbGlkUGFnZUxpbWl0YF0gZm9yIGAwYCwgW2BFcnJvcjo6UGFnZUxpbWl0RXhjZWVkZWRgXQphYm92ZSB0aGUgbWF4aW11bSkuIGBzdGFydGAgbXVzdCBiZSBgPD1gIHRoZSB3YWxsZXQncyBhdHRlc3RhdGlvbgpjb3VudCAoW2BFcnJvcjo6UGFnZVN0YXJ0T3V0T2ZSYW5nZWBdIG90aGVyd2lzZSkg4oCUIGBzdGFydGAgZXF1YWwKdG8gdGhlIGNvdW50IGlzIHZhbGlkIGFuZCByZXR1cm5zIGFuIGVtcHR5IHBhZ2UsIHRoZSBub3JtYWwKIm5vIG1vcmUgcGFnZXMiIHNpZ25hbCBmb3IgYSBjYWxsZXIgaXRlcmF0aW5nIHRvIHRoZSBlbmQuCgpSZXBsYWNlcyB2MC4xJ3MgdW5ib3VuZGVkIGBnZXRfYXR0ZXN0YXRpb25zYDogdGhpcyBjYWxsJ3MgY29zdAphbmQgcmVzcG9uc2Ugc2l6ZSBhcmUgYm91bmRlZCBieSBgbGltaXRgIHJlZ2FyZGxlc3Mgb2YgaG93IGxhcmdlCmB3YWxsZXRgJ3MgdG90YWwgaGlzdG9yeSBpcyDigJQgc2VlCmBkb2NzL2Fkci8wMDA0LXBhZ2luYXRlZC1hdHRlc3RhdGlvbi1zdG9yYWdlLm1kYC4AAAAAABVnZXRfYXR0ZXN0YXRpb25zX3BhZ2UAAAAAAAADAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAABXN0YXJ0AAAAAAAABAAAAAAAAAAFbGltaXQAAAAAAAAEAAAAAQAAA+kAAAPqAAAH0AAAAAtBdHRlc3RhdGlvbgAAAAAD",
        "AAAAAAAAAAAAAAAVZ2V0X2dpdGh1Yl9mb3Jfd2FsbGV0AAAAAAAAAQAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAQAAA+gAAAPuAAAAIA==",
        "AAAAAAAAAAAAAAAVZ2V0X3dhbGxldF9mb3JfZ2l0aHViAAAAAAAAAQAAAAAAAAAOZ2l0aHViX2lkX2hhc2gAAAAAA+4AAAAgAAAAAQAAA+gAAAAT",
        "AAAAAAAAA9pQZXJtaXNzaW9ubGVzcywgYm91bmRlZCBrZWVwLWFsaXZlIGZvciBvbmUgcGFnZSBvZiBhIHdhbGxldCdzCmF0dGVzdGF0aW9uIGhpc3Rvcnk6IGV4dGVuZHMgdGhlIFRUTCBvZiBlYWNoCmBBdHRlc3RhdGlvbkVudHJ5KHdhbGxldCwgc2VxKWAgaW4gYFtzdGFydCwgc3RhcnQrbGltaXQpYCBhbmQgdGhlCmBTZWVuUHJgIG1hcmtlciBpdHMgYHByX2hhc2hgIHBvaW50cyBhdCwgc28gYSBtZXJnZWQgUFIgY2FuIG5ldmVyCmJlY29tZSByZS1zdWJtaXR0YWJsZSBqdXN0IGJlY2F1c2UgaXRzIG1hcmtlciB3YXMgYWxsb3dlZCB0bwpleHBpcmUg4oCUIHJlZ2FyZGxlc3Mgb2Ygd2hpY2ggcGFnZSBpdCBoYXBwZW5zIHRvIGJlIG9uLgoKU2FtZSBgbGltaXRgL2BzdGFydGAgcnVsZXMgYXMKW2BTZWxmOjpnZXRfYXR0ZXN0YXRpb25zX3BhZ2VgXSAoW2BFcnJvcjo6SW52YWxpZFBhZ2VMaW1pdGBdLApbYEVycm9yOjpQYWdlTGltaXRFeGNlZWRlZGBdLCBbYEVycm9yOjpQYWdlU3RhcnRPdXRPZlJhbmdlYF0pLgpSZXR1cm5zIHRoZSBudW1iZXIgb2YgZW50cmllcyBhY3R1YWxseSByZWZyZXNoZWQ6IGEgY2FsbGVyCnN3ZWVwaW5nIGEgd2FsbGV0J3MgZnVsbCBoaXN0b3J5IGNhbGxzIHRoaXMgcmVwZWF0ZWRseSB3aXRoIGFuCmFkdmFuY2luZyBgc3RhcnRgLCBhbmQgYSByZXR1cm4gdmFsdWUgbGVzcyB0aGFuIGBsaW1pdGAKKGluY2x1ZGluZyBgMGApIHNpZ25hbHMgdGhlIHN3ZWVwIGhhcyByZWFjaGVkIHRoZSBlbmQuIENoYW5nZXMgbm8KZGF0YSDigJQgc2VlIGBkb2NzL3NlY3VyaXR5L3Jlc291cmNlLXByb2ZpbGUtdjIubWRgIGZvciB0aGUgYmFja2VuZApzY2hlZHVsaW5nIHJlc3BvbnNpYmlsaXR5IHRoaXMgaW1wbGllcywgYW5kCmBkb2NzL2Fkci8wMDA0LXBhZ2luYXRlZC1hdHRlc3RhdGlvbi1zdG9yYWdlLm1kYCBmb3Igd2h5IHRoaXMgaXMKYSBzZXBhcmF0ZSBjYWxsIGZyb20gW2BTZWxmOjpidW1wX3dhbGxldF9jb3JlX3R0bGBdLgAAAAAAGmJ1bXBfYXR0ZXN0YXRpb25zX3R0bF9wYWdlAAAAAAADAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAABXN0YXJ0AAAAAAAABAAAAAAAAAAFbGltaXQAAAAAAAAEAAAAAQAAA+kAAAAEAAAAAw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_admin: this.txFromJSON<Option<string>>,
        link_github: this.txFromJSON<Result<void>>,
        get_attestor: this.txFromJSON<Option<string>>,
        set_attestor: this.txFromJSON<Result<void>>,
        unlink_github: this.txFromJSON<Result<void>>,
        get_attestation: this.txFromJSON<Result<Attestation>>,
        submit_attestation: this.txFromJSON<Result<string>>,
        bump_wallet_core_ttl: this.txFromJSON<null>,
        get_reputation_score: this.txFromJSON<u32>,
        get_attestation_count: this.txFromJSON<u32>,
        get_attestations_page: this.txFromJSON<Result<Array<Attestation>>>,
        get_github_for_wallet: this.txFromJSON<Option<Buffer>>,
        get_wallet_for_github: this.txFromJSON<Option<string>>,
        bump_attestations_ttl_page: this.txFromJSON<Result<u32>>
  }
}