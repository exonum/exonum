'use strict';

angular.module('landTitleUi1App')
  .service('$ownersRepository', function($localStorage) {

    this.getKeypair = function() {
      if (!$localStorage.keypair) {
        $localStorage.keypair = nacl.sign.keyPair();
        return $localStorage.keypair;
      }
    };
  });
