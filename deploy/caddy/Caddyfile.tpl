{{DOMAIN}} {
    encode gzip zstd

    redir {{PANEL_BASE_PATH}} {{PANEL_BASE_PATH}}/ 308

    handle {{PANEL_BASE_PATH}}/api/* {
        uri strip_prefix {{PANEL_BASE_PATH}}
        reverse_proxy 127.0.0.1:8080
    }

    handle {{PANEL_BASE_PATH}}/s/* {
        uri strip_prefix {{PANEL_BASE_PATH}}
        reverse_proxy 127.0.0.1:8080
    }

    handle {{PANEL_BASE_PATH}}/swagger-ui* {
        uri strip_prefix {{PANEL_BASE_PATH}}
        reverse_proxy 127.0.0.1:8080
    }

    handle {{PANEL_BASE_PATH}}/api-doc/* {
        uri strip_prefix {{PANEL_BASE_PATH}}
        reverse_proxy 127.0.0.1:8080
    }

    handle_path {{PANEL_BASE_PATH}}/* {
        root * /opt/anneal/web
        try_files {path} /index.html
        file_server
    }

    respond 404
}
