<template>
  <div>
    <div class="container mt-5">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1>Latest blocks</h1>

          <ul class="list-group mt-5">
            <li class="list-group-item font-weight-bold">
              <div class="row">
                <div class="col-sm-6">Height</div>
                <div class="col-sm-6">Transactions count</div>
              </div>
            </li>
            <li v-for="block in blocks" class="list-group-item">
              <div class="row">
                <div class="col-sm-6">
                  <router-link :to="{ name: 'block', params: { height: block.height } }">{{ block.height }}</router-link>
                </div>
                <div class="col-sm-6">{{ block.tx_count }}</div>
              </div>
            </li>
          </ul>

          <button class="btn btn-lg btn-block btn-primary mt-3" @click.prevent="loadMore">Show previous blocks</button>
        </div>
      </div>
    </div>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  import Spinner from '../components/Spinner.vue'

  module.exports = {
    components: {
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
        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getBlocks(latest)
          this.blocks = this.blocks.concat(data.blocks)
          this.isSpinnerVisible = false
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      },

      loadMore() {
        this.loadBlocks(this.blocks[this.blocks.length - 1].height - 1)
      }
    },
    mounted() {
      this.$nextTick(function() {
        this.loadBlocks()
      })
    }
  }
</script>
