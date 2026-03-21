{{DOMAIN}} {
    encode gzip zstd

    handle /api/* {
        reverse_proxy 127.0.0.1:8080
    }

    handle {
        root * /opt/anneal/web
        try_files {path} /index.html
        file_server
    }
}
