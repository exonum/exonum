<template>
  <div>
    <nav class="navbar navbar-expand-lg navbar-light bg-light">
      <div class="container">
        <router-link :to="{ name: 'index' }" class="navbar-brand">
          <img src="images/exonum.png" width="41" height="36" class="align-middle mr-2" alt="">
          Timestamping
        </router-link>
        <div class="collapse navbar-collapse">
          <ul class="navbar-nav">
            <li class="nav-item">
              <router-link :to="{ name: 'blockchain' }" class="nav-link">Blockchain</router-link>
            </li>
          </ul>
        </div>
      </div>
    </nav>

    <router-view/>

    <footer class="pb-4 hr">
      <hr class="mt-5 mb-5">
      <div class="container">
        <div class="row">
          <div class="col-sm-12">
            <ul class="list-unstyled">
              <li>Source code on <a href="https://github.com/exonum/timestamping-demo" target="_blank">GitHub</a></li>
              <li>Works on <a href="https://exonum.com/doc/" target="_blank">Exonum</a></li>
            </ul>
          </div>
        </div>
      </div>
    </footer>
  </div>
</template>

<style>
  input:invalid {
    box-shadow: none;
  }

  code, .break-word {
    word-break: break-all;
    word-wrap: break-word;
  }
</style>
