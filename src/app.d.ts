// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
}

// Fontsource packages ship CSS only (no type declarations), so a bare
// side-effect `import "@fontsource-variable/manrope"` trips TS2882 under
// `noUncheckedSideEffectImports`. Declare the module so it type-checks.
declare module "@fontsource-variable/manrope";

export {};
