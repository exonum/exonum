<template>
  <div class="container">
    <div class="card-header">Голосование</div>
    <form @submit.prevent="vote">
      <div class="form-group">
        <label class="control-label">Ключ судьи:</label>
        <input
          v-model="judge_key"
          type="text"
          class="form-control"
          placeholder="Enter judge key"
          maxlength="64"
          minlength="64"
          required
        />
      </div>
      <div class="form-group">
        <label class="control-label">Ключ поединка:</label>
        <input
          v-model="duel_key"
          type="text"
          class="form-control"
          placeholder="Enter duel key"
          maxlength="64"
          minlength="64"
          required
        />
      </div>
      <div class="form-group">
        <label class="d-block">Голосовать за:</label>
        <div v-for="variant in variants" :key="variant.id" class="form-check form-check-inline">
          <input
            :id="variant.id"
            :value="variant.name"
            :checked="winner == variant.id"
            v-model="winner"
            class="form-check-input"
            type="radio"
            requred
          />
          <label :for="variant.id" class="form-check-label">{{ variant.name }}</label>
        </div>
      </div>
      <button type="submit" class="btn btn-lg btn-block btn-primary">Голосовать</button>
    </form>
  </div>
</template>

<script>
module.exports = {
  data() {
    return {
      judge_key: "",
      duel_key: "",
      winner: "",
      isSpinnerVisible: false,
      transactions: [],
      variants: [
        { id: "player1", name: "Игрок №1" },
        { id: "player2", name: "Игрок №2" },
        { id: "player3", name: "Путин" }
      ]
    };
  },
  methods: {
    async vote() {
      this.isSpinnerVisible = true;
      if (this.winner === "") {
        this.$notify("error", "Игрок не выбран");
      } else {
        try {
          //await this.$blockchain.addFunds(this.keyPair, this.amountToAdd, seed);
          this.$notify(
            "success",
            "Ключ судьи=" +
              this.judge_key +
              "\nКлюч поединка=" +
              this.duel_key +
              "\nГолос отдан за " +
              this.winner
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
