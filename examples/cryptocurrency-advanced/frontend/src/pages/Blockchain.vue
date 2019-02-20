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
  import Navbar from '../components/Navbar.vue'
  import Spinner from '../components/Spinner.vue'

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
      async loadBlocks(latest) {
        if (this.webSocket) {
          this.webSocket.close()
        }

        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getBlocks(latest)
          this.blocks = this.blocks.concat(data.blocks)
          this.isSpinnerVisible = false
          this.webSocket = new WebSocket(`ws://${window.location.host}/api/explorer/v1/blocks/subscribe`)
          this.webSocket.onmessage = this.handleNewBlock
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      },

      loadMore() {
        this.loadBlocks(this.blocks[this.blocks.length - 1].height - 1)
      },

      handleNewBlock(event) {
        this.blocks.unshift(JSON.parse(event.data))
      }
    },
    mounted() {
      this.$nextTick(function() {
        this.loadBlocks()
      })
    },
    destroyed() {
      this.webSocket.close()
    }
  }
</script>
