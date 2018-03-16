<template>
  <div>
    <router-view/>

    <footer class="pb-4 hr">
      <hr class="mt-5 mb-5">
      <div class="container">
        <div class="row">
          <div class="col-sm-12">
            <img src="images/exonum.png" width="41" height="36" class="float-left mt-sm-1 mr-3" alt="">
            <ul class="list-unstyled">
              <li>Sources on <a href="https://github.com/exonum/cryptocurrency-advanced" target="_blank">GitHub</a></li>
              <li><a href="https://exonum.com/doc/" target="_blank">Exonum docs</a></li>
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

  code {
    word-break: break-all;
    word-wrap: break-word;
  }
</style>
