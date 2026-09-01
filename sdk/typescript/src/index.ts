/**
 * @proofowl/contract-sdk — typed read-only client, transaction-prep
 * helpers, and canonical identifier helpers for the ProofOwl Soroban
 * contract.
 *
 * The deployed WASM / contract ABI is authoritative; see
 * `docs/integration/contract-api-v1.md`. This SDK never signs, submits,
 * or reads a keystore.
 */

export * from "./config.js";
export * from "./errors.js";
export * from "./client.js";
export * from "./identifiers.js";

/** The raw generated bindings (regenerated from the WASM; do not edit). */
export * as generated from "./generated/index.js";
