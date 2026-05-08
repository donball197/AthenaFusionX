
// this file is generated — do not edit it


declare module "svelte/elements" {
	export interface HTMLAttributes<T> {
		'data-sveltekit-keepfocus'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-noscroll'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-preload-code'?:
			| true
			| ''
			| 'eager'
			| 'viewport'
			| 'hover'
			| 'tap'
			| 'off'
			| undefined
			| null;
		'data-sveltekit-preload-data'?: true | '' | 'hover' | 'tap' | 'off' | undefined | null;
		'data-sveltekit-reload'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-replacestate'?: true | '' | 'off' | undefined | null;
	}
}

export {};


declare module "$app/types" {
	type MatcherParam<M> = M extends (param : string) => param is (infer U extends string) ? U : string;

	export interface AppTypes {
		RouteId(): "/" | "/admin" | "/agent-lab" | "/audit" | "/auth" | "/history" | "/secops" | "/settings" | "/todos" | "/todos/dark" | "/todos/minimal" | "/vitals" | "/workspace";
		RouteParams(): {
			
		};
		LayoutParams(): {
			"/": Record<string, never>;
			"/admin": Record<string, never>;
			"/agent-lab": Record<string, never>;
			"/audit": Record<string, never>;
			"/auth": Record<string, never>;
			"/history": Record<string, never>;
			"/secops": Record<string, never>;
			"/settings": Record<string, never>;
			"/todos": Record<string, never>;
			"/todos/dark": Record<string, never>;
			"/todos/minimal": Record<string, never>;
			"/vitals": Record<string, never>;
			"/workspace": Record<string, never>
		};
		Pathname(): "/" | "/admin" | "/agent-lab" | "/audit" | "/auth" | "/history" | "/secops" | "/settings" | "/todos/dark" | "/vitals" | "/workspace";
		ResolvedPathname(): `${"" | `/${string}`}${ReturnType<AppTypes['Pathname']>}`;
		Asset(): "/favicon.png" | string & {};
	}
}