<template>
  <div>
    <navbar/>

    <div class="container">
      <div class="row">
        <div class="col-sm-12">
          <div class="card mt-5">
            <div class="card-header">Latest blocks</div>
            <ul class="list-group list-group-flush">
              <li class="list-group-item font-weight-bold">
                <div class="row">
                  <div class="col-sm-6">Block height</div>
                  <div class="col-sm-6">Transactions count</div>
                </div>
              </li>
              <li v-for="(block) in blocks" :key="block.height" class="list-group-item">
                <div class="row">
                  <div class="col-sm-6">
                    <router-link :to="{ name: 'block', params: { height: block.height } }">{{ block.height }}</router-link>
                  </div>
                  <div class="col-sm-6">{{ block.tx_count }}</div>
                </div>
              </li>
            </ul>
            <div class="card-body text-center">
              <a href="#" class="btn btn-primary" @click.prevent="loadMore">Load older blocks</a>
            </div>
          </div>
        </div>
      </div>
    </div>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  const Navbar = require('../components/Navbar.vue')
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Navbar,
      Spinner
    },
    data() {
      return {
        isSpinnerVisible: false,
        blocks: []
      }
    },
    methods: {
      loadBlocks: function(latest) {
        const self = this

        this.isSpinnerVisible = true

        this.$blockchain.getBlocks(latest).then(data => {
          self.isSpinnerVisible = false
          self.blocks = self.blocks.concat(data)
        }).catch(error => {
          self.isSpinnerVisible = true
          self.$notify('error', error.toString())
        })
      },

      loadMore: function() {
        this.loadBlocks(this.blocks[this.blocks.length - 1].height)
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        this.loadBlocks()
      })
    }
  }
</script>
