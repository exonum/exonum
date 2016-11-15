'use strict';

angular.module('landTitleUi1App')

  .controller('TitleListCtrl', function ($scope, $uibModal, $uibModalInstance, $api) {
    $api.getTitlesList().then(function (success){
        $scope.titles = success.data;
    });

    $scope.ok = function (){
        $uibModalInstance.dismiss('');
    }
  })
  .controller('TitleInfoCtrl', function ($scope, $uibModal, $uibModalInstance, $api, $titlesManager, info) {

    $scope.titleInfo = info.titleInfo;
    $scope.center = info.center;
    var polygon = info.polygon;

    $scope.close = function () {
      $uibModalInstance.close();
    };

    $scope.reclaim = function (){
      $uibModalInstance.close();
      var title = $api.reclaimTitle($scope.titleInfo.id);
      $titlesManager.updatePolygonColors(polygon, title);
    };

    console.log($scope.titleInfo);
    $scope.transfer = function (){

      $uibModalInstance.close();

      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/title.transfer.html',
        controller: 'TitleTransferCtrl',
        size: 'lg',
        resolve: {
          info: function (){
            return info;
          }
        }
      });

    };

    $scope.transaction = new Array();

    $scope.getTransacttionDetails = function (hash){
      $api.getTransacttionDetails(hash).then(function (success){
        $scope.transaction[hash] = success.data;
      });
    }

    $scope.delete = function (){
      $uibModalInstance.dismiss('cancel');
      $api.deleteTitle($scope.titleInfo.id).then(function (success){
        $scope.titleInfo.deleted = true;
        $titlesManager.updatePolygonColors(polygon, $scope.titleInfo);
      });
    };
    $scope.restore = function (){
      $uibModalInstance.dismiss('cancel');
      $api.restoreTitle($scope.titleInfo.id).then(function (success){
        $scope.titleInfo.deleted = false;
        $titlesManager.updatePolygonColors(polygon, $scope.titleInfo);
      });
    };

  })
  .controller('TitleTransferCtrl', function ($scope, $uibModalInstance, $api, $titlesManager, info) {

    $scope.titleInfo = info.titleInfo;
    $scope.center = info.center;

    $api.getOwnersList().then(function (success){
        $scope.owners = success.data;
        $scope.newOwner = $scope.owners[info.titleInfo.owner_id];
    });


    var polygon = info.polygon;

    $scope.cancel = function (){
      $uibModalInstance.dismiss('cancel');
    };

    $scope.transfer = function () {
      if (info.titleInfo.ownerId != $scope.newOwner.id){
        var title = $api.transferTitle($scope.titleInfo.id, $scope.newOwner.id).then(function (success){
          $titlesManager.updatePolygonColors(polygon, title);
        });
      }
      $uibModalInstance.close();
    };

  });
