//! Multi-line TypeScript and React 19 / TSX fixtures for `ts_react_quirks_corpus`.

pub const OVERLOADS_AND_CTOR_TS: &str = "\
export function parseVal(x: string): number;
export function parseVal(x: number): number;
export function parseVal(x: string | number): number {
  return Number(x);
}
export class Store {
  fetch(id: string): number;
  fetch(id: string | number): number {
    return Number(id);
  }
  constructor(
    private readonly url: string,
    public active: boolean,
    protected ready: boolean,
    readonly stale: boolean,
  ) {}
}
";

pub const CLEAN_SAFETY_TS: &str = "\
export interface User { readonly name: string; }
export function safeNarrow(raw: unknown, user?: { profile?: { name: string } }, obj?: { nested: { prop?: string } }): string {
  const cfg = { mode: \"strict\" } as const;
  const checked = { name: cfg.mode } satisfies User;
  // @ts-expect-error -- testing invalid runtime payload coercion
  const forced: string = 123;
  console.error(forced);
  const parenStopped = obj ? (obj?.nested).prop! : checked.name;
  return user?.profile?.name ?? parenStopped;
}
";

pub const BAD_SAFETY_TS: &str = "\
export function hazards(raw: any, user?: { profile?: { name: string } }, x?: string): string {
  const dbl = raw as unknown as string;
  const angle = <string>raw;
  const opt1 = user?.profile!.name;
  const opt2 = (user?.profile)!.name;
  const opt3 = x!!;
  const p = new Promise(async (resolve) => { resolve(1); });
  console.log(dbl, angle, opt1, opt2, opt3, p);
  debugger;
  return \"x\";
}
// @ts-expect-error
// @ts-expect-error: short
// @ts-ignore
";

pub const VALID_REACT19_TSX: &str = "\
import React, { memo, forwardRef, use, useContext, useMemo } from \"react\";
function useful(flag: boolean): boolean { return flag; }
app.use(middleware());
export const Card = memo(function Card({ ready, ctx }: { ready: boolean; ctx: Promise<string> }) {
  const base = useContext(ThemeCtx) ?? \"light\";
  const helper = () => {
    if (!ready) return \"skip\";
    return base;
  };
  if (ready && useful(ready)) {
    const val = use(ctx);
    const val2 = React.use(ctx);
    return <section>{val}{val2}{helper()}</section>;
  }
  return null;
});
export const FancyInput = forwardRef((props: { id: string }, _ref) => {
  const memoId = useMemo(() => props.id, [props.id]);
  return <input id={memoId} />;
});
export function useFeatureFlag(raw: string): string {
  return useMemo(() => raw.trim(), [raw]);
}
";

pub const INVALID_REACT19_TSX: &str = "\
import { useEffect, useMemo, useRef, useState, use } from \"react\";
const top = use(globalPromise);
function plainHelper() {
  return useState(0);
}
export function BrokenComp({ skip, items }: { skip: boolean; items: string[] }) {
  useEffect(async () => {
    await fetchData();
  }, []);
  if (skip) {
    const [badIf] = useState(0);
    return <div>{badIf}</div>;
  }
  const afterReturn = useMemo(() => 1, []);
  const shortCircuit = skip && useRef(null);
  function UnstableChild() {
    return <span />;
  }
  const UnstableArrow = () => <div>{afterReturn}{String(shortCircuit)}</div>;
  return <div>{items.map((x) => { useRef(x); return <span key={x}>{x}</span>; })}<UnstableChild /><UnstableArrow /></div>;
}
";
