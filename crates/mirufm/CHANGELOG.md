# Changelog

## 0.1.0 (2026-07-12)


### Features

* add gpui and open an application window ([af3b938](https://github.com/atqamz/mirufm/commit/af3b938e72af34f874813758a74aa97adc6289a6))
* copy, cut, and paste with an internal clipboard ([ea4c4df](https://github.com/atqamz/mirufm/commit/ea4c4dfd73a972307b3de3d80b7b7f507be811dc))
* inline rename and new folder creation ([751bce5](https://github.com/atqamz/mirufm/commit/751bce525abd8dffed57ff23c62fd048740a04de))
* log to state dir and install a panic hook ([e9abd29](https://github.com/atqamz/mirufm/commit/e9abd29388f39880dcaf5b0133250e9a1fff2b25))
* move to trash and permanent delete with confirm ([13c2b13](https://github.com/atqamz/mirufm/commit/13c2b13954aa3dcba50b2e28182c894de2534878))
* multi-select entries with ctrl and shift click ([923aad7](https://github.com/atqamz/mirufm/commit/923aad7bdba4e7e18da7da4f9ab1b82b3135d49a))
* navigate directories across a horizontal column strip ([18f8fee](https://github.com/atqamz/mirufm/commit/18f8feecb587e8139ba7149aa71ae0185a531c84))
* preview the selected file in a pinned pane ([4e3ed56](https://github.com/atqamz/mirufm/commit/4e3ed56fa3498e53d095ed4e3935ec68220ed267))
* render current directory in a virtualized column ([2a115f3](https://github.com/atqamz/mirufm/commit/2a115f32acd084151967f938ddeea061ad0749a8))
* render per-entry git status badges in columns ([5c84889](https://github.com/atqamz/mirufm/commit/5c84889243d4db44e16f792bc30cfed77bffa15c))
* reuse cached folders and refresh columns on filesystem changes ([ae232be](https://github.com/atqamz/mirufm/commit/ae232be30fe447857c7277b4c2bae307439cb30a))
* right-click menu to open, open-with, and launch a terminal ([97b2100](https://github.com/atqamz/mirufm/commit/97b21008574be5537a40cf6ecda4eb071f5e21ce))


### Bug Fixes

* **app:** stop parking executor threads on blocking channel recv ([4574941](https://github.com/atqamz/mirufm/commit/4574941bb051f9efc47d0869b3381885f3cccb20))
* focus the root view for Escape and log spawn failures ([e05de51](https://github.com/atqamz/mirufm/commit/e05de512ab232a9452b3c2e58852080d57f47c3b))
* harden clipboard, inline rename, and delete against async reloads ([6580deb](https://github.com/atqamz/mirufm/commit/6580debf649a9a096bc9ef41b7bd4cc7555ce956))
* keep column watchers in sync when navigation is a no-op ([dc831fe](https://github.com/atqamz/mirufm/commit/dc831fe264d209a10146dc7910200bd4664e1dc1))
* keep Open With list matched to the menu that requested it ([befd2c5](https://github.com/atqamz/mirufm/commit/befd2c5960969ae729859513bdc78ac30eb4bac1))
* resolve fix-wave re-review regressions ([ae921c3](https://github.com/atqamz/mirufm/commit/ae921c3acff36de269d228ec4e5888dfdf1fea2e))
* scroll the preview pane vertically for long content ([c1b8f92](https://github.com/atqamz/mirufm/commit/c1b8f92c4e1cf40fdb3c2189290c7ff822456316))
