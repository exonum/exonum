<template>
  <div>
    <nav class="navbar navbar-expand-lg navbar-light bg-light">
      <div class="container">
        <router-link :to="{ name: 'index' }" class="navbar-brand">
          <img src="images/exonum.png" width="41" height="36" class="align-middle mr-2" alt="">
          Timestamping
        </router-link>

        <button class="navbar-toggler" type="button" data-toggle="collapse" data-target="#navbar" aria-controls="navbar" aria-expanded="false" aria-label="Toggle navigation">
          <span class="navbar-toggler-icon"></span>
        </button>

        <div class="collapse navbar-collapse" id="navbar">
          <ul class="navbar-nav">
            <li class="nav-item">
              <router-link :to="{ name: 'blockchain' }" class="nav-link">Blockchain</router-link>
            </li>
          </ul>

          <form class="form-inline ml-auto" @submit.prevent="search">
            <input v-model="hash" class="form-control" type="search" placeholder="Enter file hash" aria-label="Search">
            <button class="btn btn-outline-success ml-sm-2" type="submit">Search</button>
          </form>
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

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  import Spinner from './components/Spinner.vue'

  module.exports = {
    components: {
      Spinner
    },
    data() {
      return {
        isSpinnerVisible: false,
        hash: ''
      }
    },
    methods: {
      async search() {
        if (!this.$validate.hex(this.hash)) {
          return this.$notify('error', 'Invalid hash is passed')
        }

        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getTimestamp(this.hash)
          this.isSpinnerVisible = false
          if (data === null) {
            throw new Error('File not found')
          } else if (data.debug) {
            throw new Error(data.description)
          }
          this.$nextTick(function() {
            this.$router.push({ name: 'timestamp', params: { hash: this.hash } })
            this.hash = ''
          })
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    }
  }
</script>

<style>
  input:invalid {
    box-shadow: none;
  }

  code, .break-word {
    word-break: break-all;
    word-wrap: break-word;
  }
</style>
