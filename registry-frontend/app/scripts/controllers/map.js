'use strict';

/**
 * @ngdoc function
 * @name landTitleUi1App.controller:MainCtrl
 * @description
 * # MainCtrl
 * Controller of the landTitleUi1App
 */
angular.module('landTitleUi1App')
  .controller('MapCtrl', function ($rootScope, $scope,  NgMap, $uibModal, $titlesManager, $api) {

    var drawingManager = null;
    var chicago = new google.maps.LatLng(41.850033, -87.6500523);
    var enteredTitle = '';
    var selectedOwner = -1;
    var progressInstance = null;

    NgMap.getMap().then(function(map) {

      $scope.map = map;

      drawingManager = new google.maps.drawing.DrawingManager({
        drawingControl: false,
        map: map
      });

      google.maps.event.addListener(drawingManager, 'polygoncomplete', function(polygon){
        drawingManager.setDrawingMode(null);
        $titlesManager.registerTitle(selectedOwner, polygon, enteredTitle);
      });

      google.maps.Polygon.prototype.getBounds=function(){
        var bounds = new google.maps.LatLngBounds()
        this.getPath().forEach(function(element,index){bounds.extend(element)})
        return bounds
      }

      $titlesManager.initTitlesOnMap(map);

    });

    $rootScope.$on('titleCreateStatusStart', function (event){
      console.log('start');
      if (progressInstance == null){
          var modalInstance = $uibModal.open({
          animation: true,
          ariaLabelledBy: 'modal-title',
          ariaDescribedBy: 'modal-body',
          templateUrl: 'views/title.status.html',
          controller: function ($scope, $uibModalInstance){
              progressInstance = $uibModalInstance;
          }
        });
      }

    });

    $rootScope.$on('titleCreationFailed', function (event, data){

      console.log('failure');
      if (progressInstance != null){
        progressInstance.dismiss('');
      }

      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/title.error.html',
        controller: function ($scope, $uibModalInstance){
            $scope.result = data.result;
            $scope.ok = function (){
              $uibModalInstance.dismiss();
              failedInstance = null;
            }
        }
      });

    });

    $rootScope.$on('titleCreateStatusFinished', function (event){
      console.log('finish');
      if (progressInstance){
        progressInstance.dismiss();
        progressInstance = null;
      }
    });

    $rootScope.$on('titleInfo', function (event, info){

      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/title.info.html',
        controller: 'TitleInfoCtrl',
        size: 'lg',
        resolve: {
          info: function () {
            return {titleInfo: info.titleInfo, center: info.polygon.getBounds().getCenter(), polygon: info.polygon};
          }
        }
      });

      modalInstance.result.then(function (owner) {
      });
    });

    $scope.home = function() {
      $scope.map.setCenter(chicago);
      $scope.map.setZoom(18);
    };

    $scope.ownersList = function (){
      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/owner.list.html',
        controller: 'OwnerCtrl',
        controllerAs: 'this',
        size: 'lg',
        resolve: {
          items: function () {
            return {};
          }
        }
      });
    };

    $scope.titlesList = function (){
      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/titles.list.html',
        controller: 'TitleListCtrl',
        controllerAs: 'this',
        size: 'lg',
        resolve: {
          items: function () {
            return {};
          }
        }
      });
    };


    $scope.auth = function (){
      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/auth.html',
        controller: 'AuthCtrl',
        controllerAs: 'this',
        size: 'lg',
        resolve: {
          items: function () {
            return {};
          }
        }
      });
    };

    $scope.register = function (){
      var modalInstance = $uibModal.open({
        animation: true,
        ariaLabelledBy: 'modal-title',
        ariaDescribedBy: 'modal-body',
        templateUrl: 'views/register.html',
        controller: 'RegisterCtrl',
        controllerAs: 'this',
        size: 'lg',
        resolve: {
          items: function () {
            return {};
          }
        }
      });
    };

    $scope.createTitle = function (){
      $api.getOwnersList().then(function (success){
        var modalInstance = $uibModal.open({
          animation: true,
          ariaLabelledBy: 'modal-title',
          ariaDescribedBy: 'modal-body',
          templateUrl: 'views/title.create.info.html',
          controller: function ($scope, $uibModalInstance){
            $scope.owners = success.data;
            $scope.confirm = function (){
              if ($scope.title && $scope.owner){
                $uibModalInstance.close({title: $scope.title, owner: $scope.owner});
              }else{
                if (!$scope.title){
                  $scope.titleNotValid = true;
                }
                if (!$scope.owner){
                  $scope.ownerNotValid = true;
                }
              }

            };
            $scope.decline = function (){
              $uibModalInstance.dismiss('cancel');
            }
          },
          size: 'lg'
        });
        modalInstance.result.then(function (data) {
          enteredTitle = data.title;
          selectedOwner = data.owner.id;
          drawingManager.setDrawingMode(google.maps.drawing.OverlayType.POLYGON);
        });
      });
    };
  });
