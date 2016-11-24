'use strict';

angular.module('landTitleUi1App')
  .controller('RegisterCtrl', function($scope, $uibModalInstance, $uibModal, $api) {

    var owner = {
      firstname: '',
      lastname: ''
    };

    $scope.owner = owner;

    $scope.register = function() {

      $api.registerOwner($scope.owner).then(function(success) {
        $uibModalInstance.close($scope.owner);

        var modalInstance = $uibModal.open({
          animation: true,
          templateUrl: 'result.html',
          size: 'lg',
          controller: ['$scope', function($scope) {
            $scope.owner = owner;
            $scope.tx = success.data.tx_hash;
            $scope.success = true;
            $scope.ok = function() {
              modalInstance.dismiss('');
            };
          }]
        });
        setTimeout(function() {
          modalInstance.dismiss('')
        }, 10000);

      }, function(error) {
        $uibModalInstance.close($scope.owner);
        var modalInstance = $uibModal.open({
          animation: true,
          templateUrl: 'result.html',
          size: 'lg',
          controller: ['$scope', function($scope) {
            $scope.error = "Status code: " + error.status;
            $scope.success = false;
            $scope.ok = function() {
              modalInstance.dismiss('');
            };
          }]
        });
        setTimeout(function() {
          modalInstance.dismiss('')
        }, 10000);
      });

    };
    $scope.cancel = function() {
      $uibModalInstance.dismiss('cancel');
    };
  });
