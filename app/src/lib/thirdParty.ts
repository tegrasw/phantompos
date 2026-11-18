// The third-party components shipped in the app and their license texts
// (src/assets/third-party-licenses.json, written by
// scripts/generate-licenses.mjs). The JSON is ~260 KB, so it is loaded on
// demand, as its own chunk, when the "Third-party licenses" view opens.

export type ComponentKind = "crate" | "npm" | "font";

export interface LicenseText {
  /** SPDX identifier ("MIT", "Apache-2.0", "OFL-1.1"). */
  id: string;
  name: string;
  text: string;
}

export interface ThirdPartyComponent {
  name: string;
  version: string;
  kind: ComponentKind | (string & {});
  /** SPDX expression as declared by the component ("MIT OR Apache-2.0"). */
  license: string;
  /** Indices into `licenses`: the texts that apply to this component. */
  licenseRefs: number[];
  /** Repository or home page (shown as text; the app does not open it). */
  url?: string;
}

export interface ThirdPartyLicenses {
  licenses: LicenseText[];
  components: ThirdPartyComponent[];
}

let pending: Promise<ThirdPartyLicenses> | null = null;

/** Loads the list once; a failed load can be retried. */
export function loadThirdPartyLicenses(): Promise<ThirdPartyLicenses> {
  pending ??= import("../assets/third-party-licenses.json").then(
    (module) => module.default,
    (error: unknown) => {
      pending = null;
      throw error;
    },
  );
  return pending;
}
