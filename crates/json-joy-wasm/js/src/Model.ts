/**
 * Model — a JSON CRDT document backed by a Rust/WASM engine.
 *
 * Mirrors `Model` from `json-joy` (`json-crdt/model/Model.ts`).
 * The TypeScript class is a thin wrapper around the generated WASM class;
 * all document state lives in Rust.
 */

import { ModelApi } from './ModelApi';
import { Patch } from './Patch';
import type { WasmModel } from './nodes';

/**
 * Static/constructor interface of the Rust-generated WASM `Model` class.
 *
 * wasm-bindgen exports the `create` function as both the JS constructor
 * (`new WasmModel(sid?)`) and, when `js_name = "create"` is set, as a
 * static factory (`WasmModel.create(sid?)`).  We declare both so the wrapper
 * works regardless of the exact wasm-bindgen output.
 *
 * `rndSid` is a static method in Rust (`pub fn rnd_sid()` without `&self`),
 * so it appears as `WasmModel.rndSid()` on the class, not on instances.
 */
export interface WasmModelClass {
  /** Constructor form: `new WasmModel(sid?)`. */
  new (sid?: bigint): WasmModel;
  /** Static factory form: `WasmModel.create(sid?)`. */
  create(sid?: bigint): WasmModel;
  /** Decode from binary structural encoding. */
  fromBinary(data: Uint8Array): WasmModel;
  /** Generate a random session ID (static on the WASM class). */
  rndSid(): bigint;
}

/**
 * A JSON CRDT document.
 *
 * @example
 * ```ts
 * import init, { Model as WasmModel } from '../pkg/json_joy_wasm';
 * import { Model } from './Model';
 *
 * await init();
 * Model.init(WasmModel);
 *
 * const model = Model.create();
 * model.api.set({ greeting: '' });
 * model.api.str(['greeting']).ins(0, 'Hello, world!');
 * const patch = model.api.flush();
 * console.log(model.view()); // { greeting: 'Hello, world!' }
 * ```
 *
 * @category Document
 */
export class Model {
  // ── Initialisation ─────────────────────────────────────────────────────────

  private static _WasmModel: WasmModelClass | null = null;

  /**
   * Register the WASM-generated `Model` class.  Must be called once after the
   * WASM package has been initialised (i.e. after `await init()`).
   *
   * ```ts
   * import init, { Model as WasmModel } from '../pkg/json_joy_wasm';
   * import { Model } from './Model';
   *
   * await init();
   * Model.init(WasmModel);
   * ```
   */
  static init(WasmModelClass: WasmModelClass): void {
    Model._WasmModel = WasmModelClass;
  }

  private static requireWasm(): WasmModelClass {
    if (!Model._WasmModel) {
      throw new Error(
        'WASM not initialised — call Model.init(WasmModel) after await init()',
      );
    }
    return Model._WasmModel;
  }

  // ── Static factory methods ─────────────────────────────────────────────────

  /**
   * Generate a random session ID.
   *
   * Mirrors `Model.sid()` from the upstream library.  Generate once per user
   * and persist across sessions to avoid unbounded clock-table growth.
   *
   * Note: `rndSid` is a **static** method on the WASM class.
   */
  static sid(): number {
    return Number(Model.requireWasm().rndSid());
  }

  /**
   * Create a new empty JSON CRDT document.
   *
   * @param sid Optional session ID.  Defaults to a randomly generated one.
   *
   * Mirrors `Model.create(schema?, sid?)`.
   */
  static create(sid?: number): Model {
    const WasmClass = Model.requireWasm();
    const bigSid = sid !== undefined ? BigInt(sid) : undefined;
    // Prefer the static factory if available; fall back to the constructor.
    const wasm =
      typeof (WasmClass as unknown as { create?: (s?: bigint) => WasmModel }).create === 'function'
        ? (WasmClass as unknown as { create: (s?: bigint) => WasmModel }).create(bigSid)
        : new WasmClass(bigSid);
    return new Model(wasm);
  }

  /**
   * Decode a document from its binary structural encoding.
   *
   * Mirrors `Model.fromBinary(bytes)`.
   */
  static fromBinary(bytes: Uint8Array): Model {
    const wasm = Model.requireWasm().fromBinary(bytes);
    return new Model(wasm);
  }

  // ── Instance ───────────────────────────────────────────────────────────────

  private readonly _wasm: WasmModel;
  private _api?: ModelApi;

  private constructor(wasm: WasmModel) {
    this._wasm = wasm;
  }

  // ── Properties ─────────────────────────────────────────────────────────────

  /**
   * The editing API for this document.  Use it to apply local changes.
   *
   * ```ts
   * model.api.set({ hello: '' });
   * model.api.str(['hello']).ins(0, 'world');
   * const patch = model.api.flush();
   * ```
   */
  get api(): ModelApi {
    return (this._api ??= new ModelApi(this._wasm));
  }

  /**
   * The session ID of the local logical clock.
   */
  get sid(): number {
    return Number(this._wasm.sid());
  }

  // ── Serialisation ──────────────────────────────────────────────────────────

  /**
   * Encode this document to its binary structural representation.
   *
   * Mirrors `model.toBinary()`.
   */
  toBinary(): Uint8Array {
    return this._wasm.toBinary();
  }

  // ── View ───────────────────────────────────────────────────────────────────

  /**
   * Return the current JSON view of this document.
   *
   * Mirrors `model.view()`.
   */
  view(): unknown {
    return this._wasm.view();
  }

  // ── Forking ────────────────────────────────────────────────────────────────

  /**
   * Create a copy of this document with a new session ID.
   *
   * The fork shares the same CRDT history but will diverge from this point.
   * All local changes accumulated in the original's `api` are **not** carried
   * over — call `model.api.flush()` first if you need them.
   *
   * Mirrors `model.fork(sid?)`.
   */
  fork(sid?: number): Model {
    const wasm = this._wasm.fork(sid !== undefined ? BigInt(sid) : undefined);
    return new Model(wasm);
  }

  /**
   * Generate a fresh session ID that doesn't collide with any peer already in
   * this document.
   *
   * Mirrors `model.rndSid()`.
   *
   * Note: the upstream generates a per-document random SID; ours delegates to
   * the static `WasmModel.rndSid()` since the Rust implementation doesn't
   * require the document index for uniqueness checking in this MVP.
   */
  rndSid(): number {
    return Number(Model.requireWasm().rndSid());
  }

  // ── Patch application ──────────────────────────────────────────────────────

  /**
   * Apply a remote patch received from a peer.
   *
   * Accepts either a {@link Patch} wrapper or a raw `Uint8Array`.
   *
   * Mirrors `model.applyPatch(patch)`.
   */
  applyPatch(patch: Patch | Uint8Array): void {
    const bytes = patch instanceof Patch ? patch.bin : patch;
    this._wasm.applyPatch(bytes);
  }

  /**
   * Apply multiple remote patches in order.
   */
  applyBatch(patches: Array<Patch | Uint8Array>): void {
    for (const p of patches) this.applyPatch(p);
  }

  // ── Lifecycle ──────────────────────────────────────────────────────────────

  /**
   * Release the WASM memory held by this document.
   *
   * For typical long-lived documents (one model per open tab or session) you
   * never need to call this — just let the object be garbage-collected when
   * the page unloads.
   *
   * Call it when creating many short-lived models (server-side batch
   * processing, tests) to prevent WASM heap growth, since the JS garbage
   * collector cannot see into WASM linear memory.
   */
  dispose(): void {
    this._wasm.free();
  }
}
