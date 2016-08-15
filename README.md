# Exonum

Тут будет короткое описание

# About

Более подробное описание

# Build dependencies

## System Libraries

### Linux

Для debian based систем понадобятся следующие пакеты:
```
apt install build-essential git libsodium-dev
```

### MacOS

Прежде всего необходимо установить и настроить homebrew согласно его [инструкции](http://brew.sh/). После чего установить следующие пакеты:
```
brew install libsodium
```

## Rust

В проекте используется нестабильная ветка, для управления которой существует утилита [rustup](https://www.rustup.rs/).
Для того, чтобы установить ее и заодно нужный для сборки проекта `toolchain` достаточно выполнить следующую команду:
```
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
```

# Build instruction

Сам проект поделен на три части, каждая из которых находится в собственной директории:
 * exonum - ядро
 * sanbox - тестовые приложения
 * timestamping - реализация timestamping сервиса

### Пример команды для сборки exonum:
```
cargo build --manifest-path exonum/Cargo.toml
```

### Сборка тестовых приложений:

Пример для сборки узла тестовой сети:
```
cargo build --manifest-path sandbox/Cargo.toml --example merkle_map
```

# Развертка тестовой сети

## Вручную

Для развертки сети используется утилита `test_node`. В первую очередь нужно сгенерировать шаблонный конфиг командой,
в которой параметр `N` - это количество узлов в тестовой сети.
```
$ test_node --config ~/.config/exonum/test.toml generate N
```
Для запуска узла тестовой сети нужно указать его номер (от `0` до `N-1`):
```
$ test_node --config ~/.config/exonum/test.toml run 1
```
Можно задать список узлов, которые он изначально знает (в противном случае он будет знать все).
Номера валидаторов задаются в кавычках через пробел:
```
$ test_node --config ~/.config/exonum/test.toml run 1 --known-peers "1 2 3"
```
Для каждого тестового узла можно задать хранилище, в котором он будет хранить данные.
Если оно не будет задано, то подразумевается, что все данные хранятся в памяти.
```
$ test_node --config ~/.config/exonum/test.toml run --leveldb-path "/var/tmp/exonum/1" 1
```
Опции можно комбинировать, для получения более подробной информации можно вызвать `help`
```
test_node --help
```
Или конкретно для команды:
```
test_node run --help
```

## Автоматически

TODO
