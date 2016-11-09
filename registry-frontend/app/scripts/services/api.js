'use strict';

angular.module('landTitleUi1App')
  .service('$api', function($http, $cookies, apiUrl, apiBlockchainUrl) {

    this.registerUser = function (user){
      return $http.post(apiUrl + '/register', user, {withCredentials: true});
    };

    this.registerOwner = function (owner) {
      return $http.post(apiUrl + '/owners', owner, {withCredentials: true});
    };

    this.getOwnersList = function (){
      return $http.get(apiUrl + '/owners', {withCredentials: true});
    };

    this.getTransactionStatus = function (hash){
      return $http.get(apiUrl + '/result/' + hash, {withCredentials: true});
    };

    this.getTitlesList = function (){
      return $http.get(apiUrl + '/objects', {withCredentials: true});
    };

    this.listOwners = function () {
      return $ownersRepository.list();
    };

    this.addTitle = function (ownerId, points, title){
      return $http.post(apiUrl + '/objects', {owner_id: ownerId, points: points, title: title, deleted: false}, {withCredentials: true});
    };

    this.reclaimTitle = function (titleId){
      // var title = $titlesRepository.getTitleById(titleId);
      // title.ownerId = null;
      // $titlesRepository.update(title);
      // $historyRepository.register(titleId, 'Reclaim', {});
      // return title;
    };

    this.deleteTitle = function (id){
      return $http.delete(apiUrl + "/objects/" + id + '?rnd=' + Math.random(), {withCredentials: true});
    };

    this.restoreTitle = function (id){
      return $http.post(apiUrl + "/objects/restore", {id: id, rnd: Math.random()}, {withCredentials: true});
    };

    this.getTitleById = function (id) {
      return $http.get(apiUrl + '/objects/' + id, {withCredentials: true});
    };

    this.transferTitle = function (titleId, newOwnerId)
    {
      return $http.post(apiUrl + '/objects/transfer', {id: titleId, owner_id: newOwnerId, rnd: Math.random()}, {withCredentials: true});
    };

    this.getTransacttionDetails = function (hash){
      return $http.get(apiBlockchainUrl + '/transactions/' + hash);
    }

  });
