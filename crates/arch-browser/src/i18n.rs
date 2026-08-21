#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Language {
    Chinese,
    English,
}

impl Language {
    pub(crate) fn system() -> Self {
        Self::from_locale(sys_locale::get_locale().as_deref())
    }

    pub(crate) fn from_locale(locale: Option<&str>) -> Self {
        match locale {
            Some(locale) if locale.to_ascii_lowercase().starts_with("zh") => Self::Chinese,
            _ => Self::English,
        }
    }

    pub(crate) const fn invalid_input(self) -> &'static str {
        self.select("输入无效", "Invalid input")
    }

    pub(crate) const fn application_error(self) -> &'static str {
        self.select("应用程序错误", "Application error")
    }

    pub(crate) const fn unsupported_address(self) -> &'static str {
        self.select("不支持的地址", "Unsupported address")
    }

    pub(crate) const fn resource_too_large(self) -> &'static str {
        self.select("资源过大", "Resource too large")
    }

    pub(crate) const fn file_unavailable(self) -> &'static str {
        self.select("文件不可用", "File unavailable")
    }

    pub(crate) const fn request_timed_out(self) -> &'static str {
        self.select("请求超时", "Request timed out")
    }

    pub(crate) const fn certificate_validation_failed(self) -> &'static str {
        self.select("证书验证失败", "Certificate validation failed")
    }

    pub(crate) const fn connection_failed(self) -> &'static str {
        self.select("连接失败", "Connection failed")
    }

    pub(crate) const fn http_request_failed(self) -> &'static str {
        self.select("HTTP 请求失败", "HTTP request failed")
    }

    pub(crate) const fn secure_network_request_failed(self) -> &'static str {
        self.select("安全网络请求失败", "Secure network request failed")
    }

    pub(crate) const fn rendering_failed(self) -> &'static str {
        self.select("渲染失败", "Rendering failed")
    }

    pub(crate) const fn document_parsing_failed(self) -> &'static str {
        self.select("文档解析失败", "Document parsing failed")
    }

    pub(crate) const fn default_space_name(self) -> &'static str {
        self.select("开始", "Start")
    }

    pub(crate) fn new_space_name(self, number: usize) -> String {
        match self {
            Self::Chinese => format!("空间 {number}"),
            Self::English => format!("Space {number}"),
        }
    }

    pub(crate) const fn address_placeholder(self) -> &'static str {
        self.select("搜索或输入地址", "Search or enter an address")
    }

    pub(crate) const fn space_name_placeholder(self) -> &'static str {
        self.select("空间名称", "Space name")
    }

    pub(crate) const fn space_name_empty(self) -> &'static str {
        self.select("空间名称不能为空", "Space name cannot be empty")
    }

    pub(crate) const fn selected_space_missing(self) -> &'static str {
        self.select("所选空间已不存在", "Selected Space no longer exists")
    }

    pub(crate) const fn new_tab(self) -> &'static str {
        self.select("新建标签页", "New tab")
    }

    pub(crate) const fn close_tab(self) -> &'static str {
        self.select("关闭标签页", "Close tab")
    }

    pub(crate) const fn switch_space(self) -> &'static str {
        self.select("切换空间", "Switch Space")
    }

    pub(crate) const fn new_space(self) -> &'static str {
        self.select("新建空间", "New Space")
    }

    pub(crate) const fn rename_space(self) -> &'static str {
        self.select("重命名空间", "Rename Space")
    }

    pub(crate) const fn delete_space(self) -> &'static str {
        self.select("删除空间", "Delete Space")
    }

    pub(crate) const fn save(self) -> &'static str {
        self.select("保存", "Save")
    }

    pub(crate) const fn cancel(self) -> &'static str {
        self.select("取消", "Cancel")
    }

    pub(crate) const fn go_back(self) -> &'static str {
        self.select("后退", "Back")
    }

    pub(crate) const fn go_forward(self) -> &'static str {
        self.select("前进", "Forward")
    }

    pub(crate) const fn reload(self) -> &'static str {
        self.select("重新加载", "Reload")
    }

    pub(crate) const fn navigate(self) -> &'static str {
        self.select("前往", "Go")
    }

    pub(crate) const fn bookmark_current_page(self) -> &'static str {
        self.select("收藏当前页面", "Bookmark this page")
    }

    pub(crate) const fn remove_bookmark(self) -> &'static str {
        self.select("删除书签或文件夹", "Remove bookmark or folder")
    }

    pub(crate) const fn rename_bookmark(self) -> &'static str {
        self.select("重命名书签或文件夹", "Rename bookmark or folder")
    }

    pub(crate) const fn bookmark_name_empty(self) -> &'static str {
        self.select(
            "书签或文件夹名称不能为空",
            "Bookmark or folder name cannot be empty",
        )
    }

    pub(crate) const fn bookmarks(self) -> &'static str {
        self.select("书签", "Bookmarks")
    }

    pub(crate) const fn bookmark_bar(self) -> &'static str {
        self.select("书签栏", "Bookmarks bar")
    }

    pub(crate) const fn bookmark_folder(self) -> &'static str {
        self.select("书签文件夹", "Bookmark folder")
    }

    pub(crate) const fn new_bookmark_folder(self) -> &'static str {
        self.select("新建书签文件夹", "New bookmark folder")
    }

    pub(crate) const fn folder_name_placeholder(self) -> &'static str {
        self.select("文件夹名称", "Folder name")
    }

    pub(crate) const fn folder_name_empty(self) -> &'static str {
        self.select("文件夹名称不能为空", "Folder name cannot be empty")
    }

    pub(crate) const fn empty_folder(self) -> &'static str {
        self.select("空文件夹", "Empty folder")
    }

    pub(crate) const fn save_to_this_folder(self) -> &'static str {
        self.select("保存到此文件夹", "Save to this folder")
    }

    pub(crate) fn new_folder_name(self, number: usize) -> String {
        match self {
            Self::Chinese => format!("文件夹 {number}"),
            Self::English => format!("Folder {number}"),
        }
    }

    pub(crate) const fn open_page_to_begin(self) -> &'static str {
        self.select("打开一个页面以开始浏览", "Open a page to begin")
    }

    pub(crate) const fn diagnostics(self) -> &'static str {
        self.select("诊断信息", "Diagnostics")
    }

    pub(crate) const fn address_empty(self) -> &'static str {
        self.select("地址不能为空", "Address cannot be empty")
    }

    pub(crate) fn invalid_url(self, error: url::ParseError) -> String {
        match self {
            Self::Chinese => format!("URL 无效：{error}"),
            Self::English => format!("Invalid URL: {error}"),
        }
    }

    pub(crate) const fn invalid_file_path(self) -> &'static str {
        self.select(
            "路径无法表示为文件 URL",
            "Path cannot be represented as a file URL",
        )
    }

    pub(crate) fn invalid_address(self, address: &str) -> String {
        match self {
            Self::Chinese => format!("地址无效或路径不存在：{address}"),
            Self::English => format!("Invalid address or missing path: {address}"),
        }
    }

    const fn select(self, chinese: &'static str, english: &'static str) -> &'static str {
        match self {
            Self::Chinese => chinese,
            Self::English => english,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_locales_select_chinese() {
        for locale in ["zh-CN", "zh-Hans", "zh-TW", "ZH_hant"] {
            assert_eq!(Language::from_locale(Some(locale)), Language::Chinese);
        }
    }

    #[test]
    fn non_chinese_and_missing_locales_select_english() {
        for locale in [Some("en-US"), Some("ja-JP"), Some("fr"), None] {
            assert_eq!(Language::from_locale(locale), Language::English);
        }
    }

    #[test]
    fn exposes_localized_static_and_dynamic_copy() {
        assert_eq!(Language::Chinese.address_placeholder(), "搜索或输入地址");
        assert_eq!(
            Language::English.address_placeholder(),
            "Search or enter an address"
        );
        assert_eq!(Language::Chinese.new_space_name(3), "空间 3");
        assert_eq!(Language::English.new_space_name(3), "Space 3");
        assert_eq!(Language::Chinese.save_to_this_folder(), "保存到此文件夹");
        assert_eq!(
            Language::English.save_to_this_folder(),
            "Save to this folder"
        );
    }
}
