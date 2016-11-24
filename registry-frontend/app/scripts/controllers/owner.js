'use strict';

angular.module('landTitleUi1App')
  .controller('OwnerCtrl', function($scope, $uibModalInstance, $uibModal, $api) {
    $api.getOwnersList().then(function(success) {
      $scope.owners = success.data;
    });

    $scope.ok = function() {
      $uibModalInstance.dismiss('');
    }
  });
