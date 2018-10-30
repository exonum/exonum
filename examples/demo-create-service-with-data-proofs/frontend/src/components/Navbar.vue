<template>
  <nav class="navbar navbar-expand-lg navbar-light bg-light">
    <div class="container">
      <router-link :to="{ name: isAuthorized ? 'user' : 'home' }" class="navbar-brand">Cryptocurrency</router-link>
      <button class="navbar-toggler" type="button" data-toggle="collapse" data-target="#navbar" aria-controls="navbar" aria-expanded="false" aria-label="Toggle navigation">
        <span class="navbar-toggler-icon"/>
      </button>
      <div id="navbar" class="collapse navbar-collapse">
        <ul class="navbar-nav">
          <li class="nav-item">
            <router-link :to="{ name: 'blockchain' }" class="nav-link">Blockchain</router-link>
          </li>
        </ul>
        <ul v-if="isAuthorized" class="navbar-nav ml-auto">
          <li class="nav-item">
            <a href="#" class="nav-link" @click="logout">Logout</a>
          </li>
        </ul>
      </div>
    </div>
  </nav>
</template>

<script>
  module.exports = {
    name: 'navbar',
    computed: {
      isAuthorized() {
        return this.$store.state.keyPair !== null
      }
    },
    methods: {
      logout() {
        this.$store.commit('logout')
        this.$router.push({ name: 'home' })
      }
    }
  }
</script>
