<template>
  <div class="container">
    <div class="card-header">Голосование</div>
    <form @submit.prevent="vote">
      <div class="form-group">
        <label class="control-label">Ключ поединка:</label>
        <input v-model="duel_key" type="text" class="form-control" placeholder="Enter duel key" maxlength="64" minlength="64" required>
      </div>
      <div class="form-group">
        <label class="d-block">Голосовать за:</label>
        <div class="form-group">
          <label class="control-label">Ключ игрока:</label>
          <input v-model="player_key" type="text" class="form-control" placeholder="Enter player key" maxlength="64" minlength="64" required>
        </div>
        <!--
        <div v-for="variant in variants" :key="variant.id" class="form-check form-check-inline">
          <input :id="variant.id" :value="variant.id" :checked="winner == variant.id" v-model="winner" class="form-check-input" type="radio" requred>
          <label :for="variant.id" class="form-check-label">{{ variant.name }}</label>
        </div>
        -->
      </div>
      <button type="submit" class="btn btn-lg btn-block btn-primary">Голосовать</button>
    </form>
  </div>
</template>

<script>
import { mapState } from 'vuex'
module.exports = {
  data() {
    return {
      duel_key: "8f2a7c250858df392a0cbd149c211b0130b0e93a3cd2fe775121b1bc61b2399d",
      player_key: "",
      isSpinnerVisible: false,
      transactions: [],
      variants: [
        { id: "12349e278bb2e5f568239fa5533459ae128534eee44486bc7a41a1d538305eb1", name: "Игрок №1" },
        { id: "beb41d727f1d9e2c2dc6333ffb43052931e6a7686d1abbc6e68fa58c2b05060c", name: "Игрок №2" }
      ]
    };
  },
  computed: Object.assign({
    reverseTransactions() {
      return this.transactions.slice().reverse()
    }
  }, mapState({
     keyPair: state => state.keyPair
  })),
  methods: {
    async vote() {
      this.isSpinnerVisible = true;
      if (this.player_key === "") {
        this.$notify("error", "Игрок не выбран");
      } else {
        try {
          await this.$blockchain.createVote(this.keyPair, this.duel_key, this.player_key);
          this.$notify(
            "success",
              "Ключ поединка=" +
              this.duel_key +
              "\nГолос отдан за " +
              this.player_key
          );
        } catch (error) {
          this.isSpinnerVisible = false;
          this.$notify("error", error.toString());
        }
      }
    }
  }
};
</script>
