'use strict';

angular.module('landTitleUi1App')
  .service('$titlesManager', function($rootScope, $api) {

    var self = this;

    this.polygons = new Array();
    this.map = null;

    this.addInfoListener = function(polygon) {
      google.maps.event.addListener(polygon, 'click', function(event) {
        $api.getTitleById(polygon.id).then(function(success) {
          $rootScope.$broadcast('titleInfo', {titleInfo: success.data, polygon: polygon});
        });

      });
    };

    this.updatePolygonColors = function(polygon, title) {
      if (title.deleted) {
        polygon.setOptions({strokeColor: '#aa0000', fillColor: '#aa0000'});
      } else if (title.ownerId == null) {
        polygon.setOptions({strokeColor: '#0000aa', fillColor: '#0000aa'});
      } else {
        polygon.setOptions({strokeColor: '#00aa00', fillColor: '#00aa00'});
      }
    };

    this.addPolygon = function(polygon) {
      polygon.id = this.polygons.length;
      polygon.setMap(this.map);
      this.polygons.push(polygon);
    };

    this.removePolygon = function(polygon) {
      this.polygons.splice(polygon.id, 1);
      polygon.setMap(null);
    };

    this.initTitlesOnMap = function(map) {

      this.map = map;

      $api.getTitlesList().then(function(success) {

        console.log(success);
        if (success.error) {
          alert(success.error);
        } else {
          var items = success.data;

          items.forEach(function(title, index) {

            var points = title.points.map(function(item) {
              return {lat: item.x, lng: item.y};
            });

            var polygon = new google.maps.Polygon({
              paths: points,
              id: title.id
            });

            self.updatePolygonColors(polygon, title);
            self.addPolygon(polygon);
            self.addInfoListener(polygon);
          });
        }

      }, function(error) {
        console.log(error);
      });

    };

    this.refreshStatus = function(polygon) {
      $api.getTransactionStatus(polygon.hash).then(function(success) {
        var result = success.data.result;
        if (result == 1) {
          self.updatePolygonColors(polygon, polygon.title);
          self.addInfoListener(polygon);
          $rootScope.$broadcast('titleCreateStatusFinished');
        } else {
          $rootScope.$broadcast('titleCreationFailed', {result: result});
          self.removePolygon(polygon);
        }
      }, function(error) {
        if (polygon.retries-- > 0) {
          console && console.log("Retry info for polygon: " + polygon.id + ". Hash: " + polygon.hash);
          setTimeout(function() {
            self.refreshStatus(polygon);
          }, 3000);
        } else {
          $rootScope.$broadcast('titleCreationFailed', {result: -1});
        }
      });
    };

    this.registerTitle = function(ownerId, polygon, title) {

      var points = [];

      polygon.getPath().forEach(function(element, index) {
        points.push({
          x: element.lat(),
          y: element.lng()
        });
      });
      $api.addTitle(ownerId, points, title).then(function(success) {
        $rootScope.$broadcast('titleCreateStatusStart');
        var title = success.data;
        polygon.hash = title.tx_hash;
        polygon.retries = 5;
        polygon.title = title;
        self.addPolygon(polygon);
        self.refreshStatus(polygon);
      });

    };

  });
