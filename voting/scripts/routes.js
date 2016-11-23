(function() {
    'use strict';

    angular
        .module('votingApp')
        .config(function($stateProvider, $urlRouterProvider, constants) {
            $urlRouterProvider.otherwise("/welcome");

            $stateProvider
                .state('inner', {
                    abstract: true,
                    templateUrl: 'partials/inner.html',
                    controller: function($scope, $rootScope) {
                        $scope.back = function() {
                            $rootScope.back();
                        }
                    }
                })
                .state('welcome', {
                    url: '/welcome',
                    templateUrl: 'partials/welcome.html'
                })
                .state('monitor', {
                    parent: 'inner',
                    url: '/monitor',
                    templateUrl: 'partials/monitor.html',
                    title: 'Monitor election',
                    backState: 'welcome'
                })
                .state('elections', {
                    parent: 'inner',
                    url: '/elections',
                    templateUrl: 'partials/elections.html',
                    title: 'e-Voting',
                    backState: 'welcome',
                    electionWizard: true
                })
                .state('candidates', {
                    parent: 'inner',
                    url: '/elections/candidates',
                    templateUrl: 'partials/candidates.html',
                    title: 'Candidates of Election',
                    backState: 'elections',
                    electionWizard: true
                })
                .state('ballot', {
                    parent: 'inner',
                    url: '/elections/ballot',
                    templateUrl: 'partials/ballot.html',
                    title: 'Your Unsigned Ballot',
                    backState: 'candidates',
                    electionWizard: true
                })
                .state('signed', {
                    parent: 'inner',
                    url: '/elections/signed',
                    templateUrl: 'partials/signed.html',
                    title: 'Ballot has been signed',
                    electionWizard: true
                })
                .state('submitted', {
                    parent: 'inner',
                    url: '/elections/submitted',
                    templateUrl: 'partials/submitted.html',
                    title: 'Ballot has been submitted to voting server',
                    electionWizard: true
                })
                .state('decrypted', {
                    parent: 'inner',
                    url: '/elections/decrypted',
                    templateUrl: 'partials/decrypted.html',
                    title: 'Your Decrypted Ballot',
                    electionWizard: true
                })
                .state('tallying', {
                    parent: 'inner',
                    url: '/elections/tallying',
                    templateUrl: 'partials/tallying.html',
                    title: 'Full Ballot Encryption Details',
                    backState: 'decrypted',
                    electionWizard: true
                })
                .state('randomness', {
                    parent: 'inner',
                    url: '/elections/randomness',
                    templateUrl: 'partials/randomness.html',
                    title: 'Full Ballot Encryption Details',
                    backState: 'decrypted',
                    electionWizard: true
                })
                .state('encrypted', {
                    parent: 'inner',
                    url: '/elections/encrypted',
                    templateUrl: 'partials/encrypted.html',
                    title: 'Full Ballot Encryption Details',
                    backState: 'decrypted',
                    electionWizard: true
                })
                .state('hash', {
                    parent: 'inner',
                    url: '/elections/hash',
                    templateUrl: 'partials/hash.html',
                    title: 'Full Ballot Encryption Details',
                    backState: 'decrypted',
                    electionWizard: true
                })
                .state('memo', {
                    parent: 'inner',
                    url: '/elections/memo',
                    templateUrl: 'partials/memo.html',
                    title: 'Full Ballot Encryption Details',
                    backState: 'decrypted',
                    electionWizard: true
                });
        });
})();
