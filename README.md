
a testament to the tedium through which i will slog to achieve minor aesthetic wins


## usage

- make sure u have latex installed. 
- compile everything: `minissg`
- compile one post `minissg /path/to/post.md`
- local testing: `python -m http.server 80`
- sample nginx config:

```
server {
  listen 80;
  server_name YOUR_DOMAIN;
  root /var/www/site;
  index index.html;

  error_page 404 /404.html;

  location / {
      try_files $uri $uri/ =404;
  }
}
```

