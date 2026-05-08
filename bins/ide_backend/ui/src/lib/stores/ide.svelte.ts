export const ide = $state({
    activeTab: null as string | null,
    openTabs: [] as { path: string, content: string, isDirty: boolean }[],

    mountFile(path: string, content: string) {
        if (!this.openTabs.find(t => t.path === path)) {
            this.openTabs.push({ path, content, isDirty: false });
        }
        this.activeTab = path;
    },
    updateContent(path: string, content: string) {
        const tab = this.openTabs.find(t => t.path === path);
        if (tab && tab.content !== content) {
            tab.content = content;
            tab.isDirty = true;
        }
    },
    markSaved(path: string) {
        const tab = this.openTabs.find(t => t.path === path);
        if (tab) tab.isDirty = false;
    },
    closeTab(path: string) {
        this.openTabs = this.openTabs.filter(t => t.path !== path);
        if (this.activeTab === path) {
            this.activeTab = this.openTabs.length > 0 ? this.openTabs[this.openTabs.length - 1].path : null;
        }
    }
});
