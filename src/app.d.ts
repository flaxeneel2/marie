declare global {
    interface Window {
        core: {
            invoke: (fn: string, args?: InvokeArgs) => Promise<unknown>;
        };
    }
}