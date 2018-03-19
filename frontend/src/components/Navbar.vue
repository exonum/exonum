<template>
  <nav class="navbar navbar-expand-lg navbar-light bg-light">
    <div class="container">
      <router-link v-if="isSigned" :to="{ name: 'user' }" class="navbar-brand">Cryptocurrency</router-link>
      <router-link v-else :to="{ name: 'home' }" class="navbar-brand">Cryptocurrency</router-link>
      <div class="collapse navbar-collapse">
        <ul class="navbar-nav mr-auto">
          <li v-if="isSigned" class="nav-item">
            <router-link :to="{ name: 'user' }" class="nav-link">Cabinet</router-link>
          </li>
          <li class="nav-item">
            <router-link :to="{ name: 'blockchain' }" class="nav-link">Blockchain</router-link>
          </li>
        </ul>
        <ul class="navbar-nav">
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
    data: function() {
      return {
        isSigned: false
      }
    },
    methods: {
      logout: function() {
        this.$storage.remove()
        this.$router.push({name: 'home'})
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        const self = this

        this.$storage.get().then(function(keyPair) {
          self.isSigned = true
        }).catch(function(error) {})
      })
    }
  }
</script>
