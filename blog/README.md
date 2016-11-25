# Блог на сайте Exonum

Устанавливаем зависимости:

```
npm install --production
```

Запускаем локально:

```
npm start
```

Административная панель [http://localhost:2368/ghost](http://localhost:2368/ghost). 

Запускаем в режиме production like:

```
npm start --production
```

Запущеный в режиме production like блог будет висеть на `127.0.0.1:2368`.

Эта и другие настройки хранятся в файле `config.js` в корне.

Бэкапы базы хранятся в `/etc/db-backups/blog/`.
